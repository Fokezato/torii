use crate::{db, notify, sources::anilist, sources::nyaa, state::AppState};
use chrono::{DateTime, Duration, Utc};
use std::collections::BTreeMap;
use tauri::AppHandle;

const WATCH_REMINDER_AFTER: Duration = Duration::days(3);
const DELETE_WARNING_BEFORE: Duration = Duration::days(1);

pub async fn run_once(app: &AppHandle, state: &AppState) {
    airing_today(app, state).await;
    watch_reminders(app, state).await;
    delete_warnings(app, state).await;
}

fn base_title(watch: &db::watches::Watch) -> String {
    watch.series_title.clone().unwrap_or_else(|| nyaa::split_season(&watch.title).0)
}

fn episode_label(n: i64) -> String {
    tr!("Episódio {n}", "Episode {n}")
}

fn parse(s: Option<&str>) -> Option<DateTime<Utc>> {
    s.and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc))
}

async fn airing_today(app: &AppHandle, state: &AppState) {
    let Ok(watches) = db::watches::list(&state.db).await else { return };
    let airing: Vec<&db::watches::Watch> = watches
        .iter()
        .filter(|w| w.anilist_id.is_some() && w.status.as_deref() != Some("FINISHED"))
        .collect();
    let ids: Vec<i32> = airing.iter().filter_map(|w| w.anilist_id.map(|id| id as i32)).collect();
    if ids.is_empty() {
        return;
    }
    if let Ok(summaries) = anilist::by_ids(&state.http, &ids).await {
        for s in summaries {
            if let (Some(at), Some(ep)) = (s.next_airing_at, s.next_airing_episode) {
                let _ = sqlx::query(
                    "INSERT INTO airing_notices (anilist_id, episode, airing_at) VALUES (?, ?, ?) \
                     ON CONFLICT (anilist_id, episode) DO UPDATE SET airing_at = excluded.airing_at",
                )
                .bind(s.anilist_id as i64)
                .bind(ep as i64)
                .bind(at)
                .execute(&state.db)
                .await;
            }
        }
    }

    let now = Utc::now().timestamp();
    let due: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT anilist_id, episode, airing_at FROM airing_notices WHERE notified = 0 AND airing_at <= ?",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (anilist_id, episode, airing_at) in due {
        let _ = sqlx::query("UPDATE airing_notices SET notified = 1 WHERE anilist_id = ? AND episode = ?")
            .bind(anilist_id)
            .bind(episode)
            .execute(&state.db)
            .await;
        // Missed while the app was closed for more than a day: stay quiet.
        if now - airing_at > 24 * 3600 {
            continue;
        }
        let Some(watch) = airing.iter().find(|w| w.anilist_id == Some(anilist_id)) else { continue };
        notify::notify(
            app,
            state,
            "notify_calendar",
            tr!("Estreia hoje", "Airs today"),
            format!("{} — {}", base_title(watch), episode_label(episode)),
            "info",
            watch.cover_url.clone(),
        )
        .await;
    }
}

async fn watch_reminders(app: &AppHandle, state: &AppState) {
    let cutoff = (Utc::now() - WATCH_REMINDER_AFTER).to_rfc3339();
    let rows: Vec<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT e.id, e.watch_id, e.episode_number FROM episodes e JOIN watches w ON w.id = e.watch_id \
         WHERE e.status = 'available' AND w.streaming = 0 AND e.watched_at IS NULL \
         AND COALESCE(e.watch_position_ms, 0) = 0 AND e.watch_reminded_at IS NULL \
         AND e.available_at IS NOT NULL AND e.available_at <= ?",
    )
    .bind(&cutoff)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut by_watch: BTreeMap<i64, Vec<(i64, Option<i64>)>> = BTreeMap::new();
    for (id, watch_id, number) in rows {
        by_watch.entry(watch_id).or_default().push((id, number));
    }
    let now = Utc::now().to_rfc3339();
    for (watch_id, episodes) in by_watch {
        for (id, _) in &episodes {
            let _ = sqlx::query("UPDATE episodes SET watch_reminded_at = ? WHERE id = ?")
                .bind(&now)
                .bind(id)
                .execute(&state.db)
                .await;
        }
        let Ok(watch) = db::watches::get(&state.db, watch_id).await else { continue };
        let body = match episodes.as_slice() {
            [(_, Some(n))] => tr!(
                "{} — {} está esperando você",
                "{} — {} is waiting for you",
                base_title(&watch),
                episode_label(*n)
            ),
            _ => tr!(
                "{} episódios de {} esperando você",
                "{} episodes of {} waiting for you",
                episodes.len(),
                base_title(&watch)
            ),
        };
        notify::notify(
            app,
            state,
            "notify_watch_reminder",
            tr!("Já baixou, falta assistir", "Downloaded, not watched yet"),
            body,
            "info",
            watch.cover_url.clone(),
        )
        .await;
    }
}

async fn delete_warnings(app: &AppHandle, state: &AppState) {
    let Ok(settings) = db::settings::get_all(&state.db).await else { return };
    let global: Option<i64> = settings.get("default_delete_after_days").and_then(|v| v.parse().ok());
    let Ok(episodes) = db::episodes::list_by_status(&state.db, "available").await else { return };
    let warned: Vec<i64> = sqlx::query_scalar("SELECT id FROM episodes WHERE delete_warned_at IS NOT NULL")
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let now = Utc::now();
    let mut by_watch: BTreeMap<i64, Vec<&db::episodes::Episode>> = BTreeMap::new();
    let mut watches = BTreeMap::new();
    for ep in &episodes {
        if warned.contains(&ep.id) {
            continue;
        }
        if !watches.contains_key(&ep.watch_id) {
            let Ok(w) = db::watches::get(&state.db, ep.watch_id).await else { continue };
            watches.insert(ep.watch_id, w);
        }
        let watch = &watches[&ep.watch_id];
        let Some(days) = watch.delete_after_days.or(global).filter(|d| *d > 0) else { continue };
        let Some(available_at) = parse(ep.available_at.as_deref()) else { continue };
        let deadline = available_at + Duration::days(days);
        if now >= deadline - DELETE_WARNING_BEFORE && now < deadline {
            by_watch.entry(ep.watch_id).or_default().push(ep);
        }
    }

    let stamp = now.to_rfc3339();
    for (watch_id, eps) in by_watch {
        for ep in &eps {
            let _ = sqlx::query("UPDATE episodes SET delete_warned_at = ? WHERE id = ?")
                .bind(&stamp)
                .bind(ep.id)
                .execute(&state.db)
                .await;
        }
        let watch = &watches[&watch_id];
        let body = match eps.as_slice() {
            [ep] if ep.episode_number.is_some() => tr!(
                "{} — {} será apagado amanhã",
                "{} — {} will be deleted tomorrow",
                base_title(watch),
                episode_label(ep.episode_number.unwrap())
            ),
            _ => tr!(
                "{} episódios de {} serão apagados amanhã",
                "{} episodes of {} will be deleted tomorrow",
                eps.len(),
                base_title(watch)
            ),
        };
        notify::notify(
            app,
            state,
            "notify_delete_reminder",
            tr!("Vai ser apagado em breve", "Will be deleted soon"),
            body,
            "error",
            watch.cover_url.clone(),
        )
        .await;
    }
}
