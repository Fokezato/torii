use sqlx::SqlitePool;
use std::collections::HashMap;

const DEFAULTS: &[(&str, &str)] = &[
    ("jellyfin_mode", "0"),
    ("jellyfin_url", "http://localhost:8096"),
    ("jellyfin_api_key", ""),
    ("poll_interval_minutes", "30"),
    ("default_delete_after_days", ""),
    ("paused", "0"),
    ("player_mode", "native"),
    ("player_preferred_audio_langs", ""),
    ("player_preferred_subtitle_langs", ""),
    ("player_auto_skip_intro", "0"),
    ("player_auto_skip_ending", "0"),
    ("player_auto_skip_recap", "0"),
    ("player_next_after_ending", "0"),
    ("player_mark_intro", "1"),
    ("player_mark_ending", "1"),
    ("player_mark_recap", "1"),
    ("player_ambient_light", "1"),
    ("detect_segments", "1"),
    ("auto_update_check", "1"),
    ("delete_after_watched", "0"),
    ("strip_unused_audio", "0"),
    ("downscale_resolution", "original"),
    ("close_action", "tray"),
    ("app_language", "auto"),
    ("onboarding_done", "0"),
    ("notify_master", "1"),
    ("notify_sound", "1"),
    ("notify_found", "1"),
    ("notify_calendar", "1"),
    ("notify_error", "1"),
    ("notify_complete", "1"),
    ("notify_jellyfin", "1"),
    ("notify_watch_reminder", "1"),
    ("notify_delete_reminder", "1"),
];

pub async fn seed_defaults(pool: &SqlitePool, default_library_root: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (key, value) in DEFAULTS {
        sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?, ?)")
            .bind(*key)
            .bind(*value)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('library_root', ?)")
        .bind(default_library_root)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn get_all(pool: &SqlitePool) -> Result<HashMap<String, String>, sqlx::Error> {
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT key, value FROM settings").fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|(k, v)| (k, v.unwrap_or_default()))
        .collect())
}

pub async fn update(pool: &SqlitePool, values: &HashMap<String, String>) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (key, value) in values {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
