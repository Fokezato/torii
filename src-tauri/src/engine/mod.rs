use crate::{db, jellyfin, notify, sources::nyaa, state::AppState};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

fn notify_episode_title(watch_title: &str, raw_episode_title: &str) -> String {
    let (base, _) = nyaa::split_season(watch_title);
    match nyaa::extract_episode_number(raw_episode_title) {
        Some(ep) => tr!("{base} — Episódio {ep}", "{base} — Episode {ep}"),
        None => base,
    }
}

pub async fn start_download(
    app: &AppHandle,
    state: &AppState,
    episode_id: i64,
    watch_title: &str,
    title: &str,
    magnet: &str,
    folder: &str,
    cover_url: Option<String>,
    notify_found: bool,
    file_index: Option<usize>,
) {
    let clean_title = notify_episode_title(watch_title, title);
    match state.torrent.add_download(episode_id, magnet, folder, file_index).await {
        Ok(info_hash) => {
            if let Err(e) = db::episodes::mark_downloading(&state.db, episode_id, &info_hash).await {
                state.activity.error(tr!("Erro ao salvar estado de download de \"{clean_title}\": {e}", "Failed to save download state of \"{clean_title}\": {e}"));
                return;
            }
            state.activity.info(format!("Iniciado download: {clean_title}"));
            if notify_found {
                notify::notify(
                    app,
                    state,
                    "notify_found",
                    tr!("Novo episódio encontrado", "New episode found"),
                    tr!("{clean_title} — iniciando download", "{clean_title} — starting download"),
                    "info",
                    cover_url,
                )
                .await;
            }
        }
        Err(e) => {
            let _ = db::episodes::mark_error(&state.db, episode_id, &e.to_string()).await;
            state.activity.error(tr!("Falha ao iniciar download de \"{clean_title}\": {e}", "Failed to start download of \"{clean_title}\": {e}"));
            notify::notify(
                app,
                state,
                "notify_error",
                tr!("Falha no download", "Download failed"),
                tr!("Não foi possível baixar {clean_title}", "Couldn't download {clean_title}"),
                "error",
                cover_url,
            )
            .await;
        }
    }
}

pub(crate) fn split_langs(raw: &Option<String>) -> Vec<String> {
    raw.as_deref()
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The anime's own titles, most specific first (see `nyaa::is_other_work`).
fn watch_names(watch: &db::watches::Watch) -> Vec<String> {
    let mut names = vec![nyaa::split_season(&watch.query).0, nyaa::split_season(&watch.title).0];
    names.extend(watch.series_title.clone());
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    names.dedup();
    names
}

fn nothing_matching(
    watch: &db::watches::Watch,
    rejected: &nyaa::Rejections,
    audio_langs: &[String],
    sub_langs: &[String],
) -> String {
    let mut reasons = Vec::new();
    if rejected.language > 0 {
        let mut wanted = Vec::new();
        if !audio_langs.is_empty() {
            wanted.push(tr!("áudio {}", "audio {}", audio_langs.join("/")));
        }
        if !sub_langs.is_empty() {
            wanted.push(tr!("legenda {}", "subtitles {}", sub_langs.join("/")));
        }
        reasons.push(tr!(
            "{} sem o idioma escolhido ({})",
            "{} without the chosen language ({})",
            rejected.language,
            wanted.join(", ")
        ));
    }
    if rejected.other_work > 0 {
        reasons.push(tr!(
            "{} de outra obra (filme, OVA ou outra temporada)",
            "{} from another work (movie, OVA or another season)",
            rejected.other_work
        ));
    }
    if rejected.quality > 0 {
        reasons.push(tr!("{} em outra qualidade", "{} in another quality", rejected.quality));
    }
    if rejected.season > 0 {
        reasons.push(tr!("{} de outra temporada", "{} from another season", rejected.season));
    }
    if rejected.range > 0 {
        reasons.push(tr!("{} fora do intervalo de episódios", "{} outside the episode range", rejected.range));
    }
    if reasons.is_empty() {
        return tr!("Nada compatível ainda pra \"{}\"", "Nothing matching yet for \"{}\"", watch.title);
    }
    tr!(
        "Nada compatível pra \"{}\": {}",
        "Nothing matching for \"{}\": {}",
        watch.title,
        reasons.join(", ")
    )
}

/// Pack episodes that failed to start keep their pack and file; they are retried here
/// because the pack itself is already marked as seen.
async fn retry_pack_errors(app: &AppHandle, state: &AppState, watch: &db::watches::Watch) {
    let Ok(episodes) = db::episodes::list_for_watch(&state.db, watch.id).await else { return };
    for ep in episodes.into_iter().filter(|e| e.status == "error") {
        let (Some(index), Some(magnet)) = (ep.file_index, ep.magnet_uri.as_deref()) else { continue };
        if watch.streaming {
            let _ = db::episodes::mark_ready(&state.db, ep.id).await;
            continue;
        }
        let name = ep.name.clone().unwrap_or_default();
        start_download(
            app,
            state,
            ep.id,
            &watch.title,
            &name,
            magnet,
            &watch.folder,
            watch.cover_url.clone(),
            false,
            Some(index as usize),
        )
        .await;
    }
}

/// Season packs: episodes still missing get their file from a pack (best seeded first).
async fn assign_packs(
    app: &AppHandle,
    state: &AppState,
    watch: &db::watches::Watch,
    packs: &[nyaa::NyaaCandidate],
    names: &[String],
) -> u32 {
    const MAX_PACKS: usize = 3;
    if packs.is_empty() {
        return 0;
    }
    let season = nyaa::split_season(&watch.query).1.unwrap_or(1);
    let Ok(episodes) = db::episodes::list_for_watch(&state.db, watch.id).await else { return 0 };
    let in_range = |n: i64| watch.episode_start.map_or(true, |s| n >= s) && watch.episode_end.map_or(true, |e| n <= e);
    let mut missing: std::collections::HashMap<u32, db::episodes::Episode> = episodes
        .into_iter()
        .filter(|e| matches!(e.status.as_str(), "pending" | "error"))
        .filter_map(|e| {
            let n = e.episode_number.filter(|n| in_range(*n))?;
            Some((n as u32, e))
        })
        .collect();
    if missing.is_empty() {
        return 0;
    }

    let mut packs: Vec<&nyaa::NyaaCandidate> = packs.iter().collect();
    packs.sort_by_key(|p| std::cmp::Reverse(p.seeders.unwrap_or(0)));
    let mut assigned = 0u32;
    for pack in packs.into_iter().take(MAX_PACKS) {
        if missing.is_empty() {
            break;
        }
        let files = match state.torrent.list_files(&pack.magnet).await {
            Ok(files) => files,
            Err(e) => {
                state.activity.info(tr!("Pacote sem resposta ({}): {e}", "Pack not responding ({}): {e}", pack.title));
                continue;
            }
        };
        let mut by_episode: std::collections::HashMap<u32, crate::torrent_engine::PackFile> = std::collections::HashMap::new();
        for file in files {
            if let Some(n) = nyaa::pack_file_episode(&file.path, names, season) {
                if by_episode.get(&n).map_or(true, |f| file.len > f.len) {
                    by_episode.insert(n, file);
                }
            }
        }
        let mut covered: Vec<u32> = missing.keys().filter(|n| by_episode.contains_key(n)).copied().collect();
        covered.sort_unstable();
        let _ = db::seen_items::mark_seen(&state.db, watch.id, &pack.id, &pack.title, !covered.is_empty()).await;
        if covered.is_empty() {
            continue;
        }
        state.activity.info(tr!(
            "Pacote da temporada com {} episódios de \"{}\": {}",
            "Season pack with {} episodes of \"{}\": {}",
            covered.len(),
            watch.title,
            pack.title
        ));
        for n in covered {
            let (Some(episode), Some(file)) = (missing.remove(&n), by_episode.get(&n)) else { continue };
            let name = file.path.rsplit(['/', '\\']).next().unwrap_or(&pack.title).to_string();
            if db::episodes::switch_source(&state.db, episode.id, &pack.id, &name, &pack.magnet).await.is_err() {
                continue;
            }
            let _ = db::episodes::set_file_index(&state.db, episode.id, Some(file.index as i64)).await;
            let _ = db::episode_sources::add_pack_source(&state.db, episode.id, pack, file.index as i64).await;
            let _ = db::episode_sources::set_active(&state.db, episode.id, &pack.id).await;
            if watch.streaming {
                let _ = db::episodes::mark_ready(&state.db, episode.id).await;
            } else {
                start_download(
                    app,
                    state,
                    episode.id,
                    &watch.title,
                    &name,
                    &pack.magnet,
                    &watch.folder,
                    watch.cover_url.clone(),
                    false,
                    Some(file.index),
                )
                .await;
            }
            assigned += 1;
        }
    }
    assigned
}

pub async fn poll_watch(app: &AppHandle, state: &AppState, watch: &db::watches::Watch) {
    let audio_langs = split_langs(&watch.audio_lang);
    let sub_langs = split_langs(&watch.sub_lang);

    let seen_ids = match db::seen_items::get_seen_ids(&state.db, watch.id).await {
        Ok(ids) => ids,
        Err(e) => {
            state.activity.error(format!("Erro ao checar \"{}\": {e}", watch.title));
            return;
        }
    };

    state.activity.info(tr!("Procurando episódios de \"{}\"...", "Searching episodes of \"{}\"...", watch.title));

    let names = watch_names(watch);
    let result = nyaa::find_new_matches(
        &state.http,
        &watch.query,
        &names,
        &watch.quality,
        &audio_langs,
        &sub_langs,
        watch.episode_start,
        watch.episode_end,
        &seen_ids,
    )
    .await;

    let result = match result {
        Ok(r) => r,
        Err(e) => {
            state.activity.error(format!("Falha ao procurar \"{}\": {e}", watch.title));
            return;
        }
    };

    for candidate in &result.all_new {
        let is_matched = result
            .matched
            .iter()
            .any(|m| m.primary.id == candidate.id || m.alternates.iter().any(|a| a.id == candidate.id));
        let is_pack = result.batches.iter().any(|b| b.id == candidate.id);
        if !is_matched && !is_pack {
            let _ = db::seen_items::mark_seen(&state.db, watch.id, &candidate.id, &candidate.title, false).await;
        }
    }

    let batch = result.matched.len() > 1 || !result.batches.is_empty();
    let mut started = 0u32;

    for episode_match in &result.matched {
        let candidate = &episode_match.primary;

        let _ = db::seen_items::mark_seen(&state.db, watch.id, &candidate.id, &candidate.title, true).await;
        for alt in &episode_match.alternates {
            let _ = db::seen_items::mark_seen(&state.db, watch.id, &alt.id, &alt.title, true).await;
        }

        let episode_number = nyaa::extract_episode_number(&candidate.title).map(i64::from);
        let outcome = db::episodes::add(
            &state.db,
            db::episodes::NewEpisode {
                watch_id: watch.id,
                source_item_id: &candidate.id,
                name: &candidate.title,
                magnet_uri: &candidate.magnet,
                save_path: &watch.folder,
                status: "found",
                episode_number,
            },
        )
        .await;

        match outcome {
            Ok(episode) if matches!(episode.status.as_str(), "pending" | "found" | "error") => {
                let clean_title = notify_episode_title(&watch.title, &candidate.title);
                if episode.status == "error" {
                    state.activity.info(tr!("Tentando de novo: {clean_title}", "Retrying: {clean_title}"));
                } else {
                    state.activity.info(format!("Encontrado: {clean_title}"));
                }
                if let Err(e) = db::episodes::switch_source(
                    &state.db,
                    episode.id,
                    &candidate.id,
                    &candidate.title,
                    &candidate.magnet,
                )
                .await
                {
                    state.activity.error(tr!("Erro ao salvar fonte de \"{clean_title}\": {e}", "Failed to save source of \"{clean_title}\": {e}"));
                    continue;
                }
                if let Err(e) =
                    db::episode_sources::add_many(&state.db, episode.id, candidate, &episode_match.alternates)
                        .await
                {
                    state.activity.error(tr!("Erro ao salvar fontes de \"{clean_title}\": {e}", "Failed to save sources of \"{clean_title}\": {e}"));
                }
                let _ = db::episode_sources::set_active(&state.db, episode.id, &candidate.id).await;
                if watch.streaming {
                    if let Err(e) = db::episodes::mark_ready(&state.db, episode.id).await {
                        state.activity.error(tr!("Erro ao salvar fonte de \"{clean_title}\": {e}", "Failed to save source of \"{clean_title}\": {e}"));
                        continue;
                    }
                    if !batch {
                        notify::notify(
                            app,
                            state,
                            "notify_found",
                            tr!("Novo episódio encontrado", "New episode found"),
                            tr!("{clean_title} — pronto pra assistir", "{clean_title} — ready to watch"),
                            "info",
                            watch.cover_url.clone(),
                        )
                        .await;
                    }
                } else {
                    start_download(
                        app,
                        state,
                        episode.id,
                        &watch.title,
                        &candidate.title,
                        &candidate.magnet,
                        &watch.folder,
                        watch.cover_url.clone(),
                        !batch,
                        None,
                    )
                    .await;
                }
                started += 1;
            }
            Ok(episode) if episode.status == "downloading" => {
                let candidates: Vec<&nyaa::NyaaCandidate> =
                    std::iter::once(candidate).chain(episode_match.alternates.iter()).collect();
                let _ = db::episode_sources::add_alternates(&state.db, episode.id, &candidates).await;
            }
            Ok(_) => {}
            Err(e) => state
                .activity
                .error(tr!("Erro ao salvar episódio de \"{}\": {e}", "Failed to save episode of \"{}\": {e}", watch.title)),
        }
    }

    retry_pack_errors(app, state, watch).await;
    let from_packs = assign_packs(app, state, watch, &result.batches, &names).await;
    started += from_packs;
    if result.matched.is_empty() && from_packs == 0 && !result.all_new.is_empty() {
        state.activity.info(nothing_matching(watch, &result.rejected, &audio_langs, &sub_langs));
    }

    if batch && started > 0 {
        let (base_title, _) = nyaa::split_season(&watch.title);
        let body = if watch.streaming {
            tr!("{started} episódios de {base_title} — prontos pra assistir", "{started} episodes of {base_title} — ready to watch")
        } else {
            tr!("{started} episódios de {base_title} — iniciando download", "{started} episodes of {base_title} — starting download")
        };
        notify::notify(
            app,
            state,
            "notify_found",
            tr!("Novos episódios encontrados", "New episodes found"),
            body,
            "info",
            watch.cover_url.clone(),
        )
        .await;
    }
}

pub async fn resume_pending_downloads(app: AppHandle) {
    let app = &app;
    let state = app.state::<AppState>();
    let mut pending = Vec::new();
    for status in ["found", "downloading"] {
        match db::episodes::list_by_status(&state.db, status).await {
            Ok(mut eps) => pending.append(&mut eps),
            Err(e) => state.activity.error(tr!("Erro ao listar episódios pendentes: {e}", "Failed to list pending episodes: {e}")),
        }
    }

    let notify_individually = pending.len() <= 1;

    for ep in pending {
        let (Some(magnet), Some(save_path)) = (ep.magnet_uri.as_deref(), ep.save_path.as_deref()) else {
            continue;
        };
        let title = ep.name.as_deref().unwrap_or("episódio");
        let Ok(watch) = db::watches::get(&state.db, ep.watch_id).await else {
            continue;
        };
        start_download(
            app,
            &state,
            ep.id,
            &watch.title,
            title,
            magnet,
            save_path,
            watch.cover_url,
            notify_individually,
            ep.file_index.map(|i| i as usize),
        )
        .await;
    }
}

async fn rename_to_clean_filename(state: &AppState, episode_id: i64, watch_id: i64) -> Option<PathBuf> {
    let old_path = state.torrent.primary_file_path(episode_id).await?;
    let _ = db::episodes::set_item_path(&state.db, episode_id, &old_path.to_string_lossy()).await;
    // Files inside a season pack stay as they are: the pack is still downloading the others.
    if db::episodes::get(&state.db, episode_id).await.is_ok_and(|e| e.file_index.is_some()) {
        return Some(old_path);
    }
    let Ok(watch) = db::watches::get(&state.db, watch_id).await else {
        return Some(old_path);
    };
    let (base_title, _) = nyaa::split_season(&watch.title);
    let Some(raw_filename) = old_path.file_name().and_then(|n| n.to_str()) else {
        return Some(old_path);
    };
    let Some(clean_name) = crate::clean_filename::clean_episode_filename(&base_title, raw_filename) else {
        return Some(old_path);
    };
    let new_path = old_path.with_file_name(&clean_name);
    if new_path == old_path {
        return Some(old_path);
    }
    match tokio::fs::rename(&old_path, &new_path).await {
        Ok(()) => {
            let _ = db::episodes::set_item_path(&state.db, episode_id, &new_path.to_string_lossy()).await;
            Some(new_path)
        }
        Err(e) => {
            state.activity.error(format!("Erro ao renomear arquivo baixado: {e}"));
            Some(old_path)
        }
    }
}

async fn sync_jellyfin_after_download(app: AppHandle, episode_id: i64, file_path: PathBuf) {
    let state = app.state::<AppState>();
    let settings = match db::settings::get_all(&state.db).await {
        Ok(s) => s,
        Err(_) => return,
    };
    if settings.get("jellyfin_mode").map(String::as_str) != Some("1") {
        return;
    }
    let (Some(url), Some(api_key)) = (
        settings.get("jellyfin_url").filter(|s| !s.is_empty()),
        settings.get("jellyfin_api_key").filter(|s| !s.is_empty()),
    ) else {
        return;
    };

    let parent = file_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if let Err(e) = jellyfin::refresh_path(&state.http, url, api_key, &parent).await {
        state.activity.error(format!("Erro ao pedir refresh ao Jellyfin: {e}"));
        return;
    }

    let file_path_str = file_path.to_string_lossy().to_string();
    match jellyfin::find_item_by_path(&state.http, url, api_key, &file_path_str).await {
        Ok(Some(item_id)) => {
            let _ = db::episodes::set_jellyfin_item_id(&state.db, episode_id, &item_id).await;
            if let Ok(episode) = db::episodes::get(&state.db, episode_id).await {
                let raw_title = episode.name.clone().unwrap_or_else(|| tr!("episódio #{episode_id}", "episode #{episode_id}"));
                if let Ok(watch) = db::watches::get(&state.db, episode.watch_id).await {
                    let clean_title = notify_episode_title(&watch.title, &raw_title);
                    notify::notify(
                        &app,
                        &state,
                        "notify_jellyfin",
                        tr!("Disponível no Jellyfin", "Available on Jellyfin"),
                        clean_title,
                        "success",
                        watch.cover_url,
                    )
                    .await;
                }
            }
        }
        Ok(None) => {
            state.activity.error(tr!("Jellyfin não achou o episódio depois do refresh", "Jellyfin didn't find the episode after the refresh"));
        }
        Err(e) => {
            state.activity.error(format!("Erro ao consultar Jellyfin: {e}"));
        }
    }
}

pub async fn resync_jellyfin_library(app: AppHandle) {
    let state = app.state::<AppState>();
    let Ok(episodes) = db::episodes::list_by_status(&state.db, "available").await else {
        return;
    };
    let pending: Vec<(i64, String)> = episodes
        .into_iter()
        .filter(|e| e.jellyfin_item_id.is_none())
        .filter_map(|e| e.item_path.map(|p| (e.id, p)))
        .collect();
    drop(state);

    for (episode_id, item_path) in pending {
        sync_jellyfin_after_download(app.clone(), episode_id, PathBuf::from(item_path)).await;
    }
}

pub async fn force_download_episode(
    app: &AppHandle,
    state: &AppState,
    episode_id: i64,
    notify: bool,
) -> Result<bool, crate::error::AppError> {
    use crate::error::AppError;
    let episode = db::episodes::get(&state.db, episode_id).await?;
    let watch = db::watches::get(&state.db, episode.watch_id).await?;
    let Some(episode_number) = episode
        .episode_number
        .or_else(|| episode.name.as_deref().and_then(crate::sources::nyaa::extract_episode_number).map(i64::from))
    else {
        return Err(AppError::Fetch(tr!("episódio sem número identificável", "episode has no recognizable number")));
    };

    let audio_langs = split_langs(&watch.audio_lang);
    let sub_langs = split_langs(&watch.sub_lang);
    let best = crate::sources::nyaa::find_best_for_episode(
        &state.http,
        &watch.query,
        &watch.quality,
        &audio_langs,
        &sub_langs,
        episode_number as u32,
    )
    .await
    .map_err(AppError::Fetch)?;

    let Some(episode_match) = best else {
        state.activity.info(tr!("Forçar verificação: nada achado pra \"{}\" ep {episode_number}", "Check now: nothing found for \"{}\" ep {episode_number}", watch.title));
        return Ok(false);
    };

    let candidate = &episode_match.primary;
    db::episodes::switch_source(&state.db, episode_id, &candidate.id, &candidate.title, &candidate.magnet).await?;
    db::episode_sources::add_many(&state.db, episode_id, candidate, &episode_match.alternates).await?;
    db::episode_sources::set_active(&state.db, episode_id, &candidate.id).await?;
    start_download(
        app,
        state,
        episode_id,
        &watch.title,
        &candidate.title,
        &candidate.magnet,
        &watch.folder,
        watch.cover_url,
        notify,
        None,
    )
    .await;
    Ok(true)
}

pub async fn start_stream(app: &AppHandle, state: &AppState, episode_id: i64) -> Result<(), crate::error::AppError> {
    use crate::error::AppError;
    if state.torrent.stream_target(episode_id).await.is_some() {
        return Ok(());
    }
    let episode = db::episodes::get(&state.db, episode_id).await?;
    let watch = db::watches::get(&state.db, episode.watch_id).await?;
    let known_source = matches!(episode.status.as_str(), "ready" | "found" | "downloading")
        .then_some(episode.magnet_uri.as_deref())
        .flatten();
    match known_source {
        Some(magnet) => {
            let title = episode.name.as_deref().unwrap_or("episódio");
            start_download(
                app,
                state,
                episode_id,
                &watch.title,
                title,
                magnet,
                &watch.folder,
                watch.cover_url.clone(),
                false,
                episode.file_index.map(|i| i as usize),
            )
            .await;
        }
        None => {
            if !force_download_episode(app, state, episode_id, false).await? {
                return Err(AppError::Fetch(tr!(
                    "Nenhuma fonte encontrada pra esse episódio ainda",
                    "No source found for this episode yet"
                )));
            }
        }
    }
    if state.torrent.stream_target(episode_id).await.is_some() {
        return Ok(());
    }
    let message = db::episodes::get(&state.db, episode_id)
        .await
        .ok()
        .and_then(|e| e.error_message)
        .unwrap_or_else(|| tr!("não foi possível iniciar o torrent", "couldn't start the torrent"));
    Err(AppError::Fetch(message))
}

pub async fn prefetch_next(app: &AppHandle, state: &AppState, watch_id: i64, episode_number: i64) {
    let Ok(watch) = db::watches::get(&state.db, watch_id).await else { return };
    if !watch.streaming {
        return;
    }
    let Ok(episodes) = db::episodes::list_for_watch(&state.db, watch_id).await else { return };
    let Some(next) = episodes.into_iter().find(|e| e.episode_number == Some(episode_number + 1)) else {
        return;
    };
    if next.watched_at.is_some() || matches!(next.status.as_str(), "available" | "downloading") {
        return;
    }
    if let Err(e) = start_stream(app, state, next.id).await {
        state.activity.info(tr!(
            "Não deu pra adiantar o próximo episódio: {e}",
            "Couldn't prefetch the next episode: {e}"
        ));
    }
}

pub async fn switch_source(
    app: &AppHandle,
    state: &AppState,
    episode_id: i64,
    source_item_id: &str,
) -> Result<(), crate::error::AppError> {
    let source = db::episode_sources::get(&state.db, episode_id, source_item_id).await?;
    let _ = state.torrent.remove(episode_id, true).await;
    db::episodes::switch_source(&state.db, episode_id, &source.source_item_id, &source.title, &source.magnet_uri)
        .await?;
    db::episode_sources::set_active(&state.db, episode_id, source_item_id).await?;
    db::episodes::set_file_index(&state.db, episode_id, source.file_index).await?;

    let episode = db::episodes::get(&state.db, episode_id).await?;
    let watch = db::watches::get(&state.db, episode.watch_id).await?;
    start_download(
        app,
        state,
        episode_id,
        &watch.title,
        &source.title,
        &source.magnet_uri,
        &watch.folder,
        watch.cover_url,
        true,
        source.file_index.map(|i| i as usize),
    )
    .await;
    Ok(())
}

const STALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);

type StallTracker = std::collections::HashMap<i64, (u64, std::time::Instant)>;

async fn handle_stalled(
    app: &AppHandle,
    state: &AppState,
    episode_id: i64,
    tried: &mut std::collections::HashMap<i64, std::collections::HashSet<String>>,
) {
    let Ok(sources) = db::episode_sources::list(&state.db, episode_id).await else {
        return;
    };
    let tried_here = tried.entry(episode_id).or_default();
    for s in sources.iter().filter(|s| s.is_active == 1) {
        tried_here.insert(s.source_item_id.clone());
    }
    let Some(next) = sources.iter().find(|s| !tried_here.contains(&s.source_item_id)) else {
        return;
    };
    tried_here.insert(next.source_item_id.clone());
    let name = db::episodes::get(&state.db, episode_id)
        .await
        .ok()
        .and_then(|e| e.name)
        .unwrap_or_else(|| tr!("episódio #{episode_id}", "episode #{episode_id}"));
    state.activity.info(tr!(
        "Download parado há 10 min, trocando de fonte: {name}",
        "Download stalled for 10 min, switching source: {name}"
    ));
    if let Err(e) = switch_source(app, state, episode_id, &next.source_item_id).await {
        state.activity.error(tr!("Erro ao trocar de fonte: {e}", "Failed to switch source: {e}"));
    }
}

pub fn spawn_download_reconciler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut stall: StallTracker = StallTracker::new();
        let mut tried = std::collections::HashMap::new();
        loop {
            {
                let state = app.state::<AppState>();
                let snapshot = state.torrent.snapshot().await;

                let now = std::time::Instant::now();
                let mut stalled = Vec::new();
                stall.retain(|id, _| snapshot.iter().any(|(e, _)| e == id));
                for (episode_id, stats) in &snapshot {
                    let running = !stats.finished && stats.state.to_string() == "live";
                    let entry = stall.entry(*episode_id).or_insert((stats.progress_bytes, now));
                    if !running || stats.progress_bytes != entry.0 {
                        *entry = (stats.progress_bytes, now);
                    } else if now.duration_since(entry.1) >= STALL_TIMEOUT {
                        stalled.push(*episode_id);
                        *entry = (stats.progress_bytes, now);
                    }
                }
                for episode_id in stalled {
                    handle_stalled(&app, &state, episode_id, &mut tried).await;
                }

                let payload: Vec<serde_json::Value> = snapshot
                    .iter()
                    .map(|(episode_id, stats)| {
                        let live = stats.live.as_ref();
                        serde_json::json!({
                            "episode_id": episode_id,
                            "state": stats.state.to_string(),
                            "progress_bytes": stats.progress_bytes,
                            "total_bytes": stats.total_bytes,
                            "finished": stats.finished,
                            "download_speed_mbps": live.map(|l| l.download_speed.mbps),
                            "upload_speed_mbps": live.map(|l| l.upload_speed.mbps),
                            "eta_human": live.and_then(|l| l.time_remaining.as_ref()).map(|d| d.to_string()),
                            "peers": live.map(|l| l.snapshot.peer_stats.live),
                        })
                    })
                    .collect();
                let _ = app.emit("downloads:progress", payload);

                for (episode_id, stats) in snapshot {
                    if stats.finished {
                        let Ok(episode) = db::episodes::get(&state.db, episode_id).await else {
                            continue;
                        };
                        if episode.status == "downloading" {
                            let title = episode.name.clone().unwrap_or_else(|| tr!("episódio #{episode_id}", "episode #{episode_id}"));
                            let watch = db::watches::get(&state.db, episode.watch_id).await.ok();
                            let cover_url = watch.as_ref().and_then(|w| w.cover_url.clone());
                            let clean_title = watch
                                .as_ref()
                                .map(|w| notify_episode_title(&w.title, &title))
                                .unwrap_or_else(|| title.clone());
                            let final_path = rename_to_clean_filename(&state, episode_id, episode.watch_id).await;
                            if let Err(e) = db::episodes::mark_available(&state.db, episode_id).await {
                                state.activity.error(tr!("Erro ao marcar episódio disponível: {e}", "Failed to mark episode as available: {e}"));
                            } else {
                                state.activity.info(tr!("Download concluído: {clean_title}", "Download complete: {clean_title}"));
                                notify::notify(
                                    &app,
                                    &state,
                                    "notify_complete",
                                    tr!("Download concluído", "Download complete"),
                                    tr!("{clean_title} já está pronto pra assistir", "{clean_title} is ready to watch"),
                                    "success",
                                    cover_url,
                                )
                                .await;
                            }
                            crate::intro_detect::spawn_pending(&app);
                            crate::postprocess::spawn_pending(&app, currently_open_media(&app));
                            if let Some(path) = final_path {
                                tauri::async_runtime::spawn(sync_jellyfin_after_download(
                                    app.clone(),
                                    episode_id,
                                    path,
                                ));
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

pub async fn poll_once(app: &AppHandle, state: &AppState) {
    let Ok(_guard) = state.poll_lock.try_lock() else {
        return;
    };

    let settings = match db::settings::get_all(&state.db).await {
        Ok(s) => s,
        Err(_) => return,
    };
    if settings.get("paused").map(String::as_str) == Some("1") {
        return;
    }

    let watches = match db::watches::list(&state.db).await {
        Ok(w) => w,
        Err(_) => return,
    };

    for watch in watches.into_iter().filter(|w| w.active) {
        poll_watch(app, state, &watch).await;
    }
}

/// Deletes an episode on request: files, torrent and Jellyfin item. "deleted" is final,
/// so the poller never downloads it again (only "Download again" does).
pub async fn delete_episode(state: &AppState, episode_id: i64) -> Result<(), crate::error::AppError> {
    let ep = db::episodes::get(&state.db, episode_id).await?;
    let watch = db::watches::get(&state.db, ep.watch_id).await?;
    let settings = db::settings::get_all(&state.db).await.unwrap_or_default();
    let jellyfin = match (
        settings.get("jellyfin_mode").map(String::as_str) == Some("1"),
        settings.get("jellyfin_url").filter(|s| !s.is_empty()),
        settings.get("jellyfin_api_key").filter(|s| !s.is_empty()),
    ) {
        (true, Some(url), Some(key)) => Some((url, key)),
        _ => None,
    };
    let _ = state.torrent.remove(ep.id, true).await;
    let raw = ep.name.clone().unwrap_or_else(|| tr!("episódio #{}", "episode #{}", ep.id));
    let clean_title = notify_episode_title(&watch.title, &raw);
    if !remove_episode(state, &ep, &clean_title, jellyfin).await {
        return Err(crate::error::AppError::Fetch(tr!(
            "Não foi possível excluir o episódio",
            "Couldn't delete the episode"
        )));
    }
    state.activity.info(tr!("Episódio excluído: {clean_title}", "Episode deleted: {clean_title}"));
    Ok(())
}

async fn remove_episode(
    state: &AppState,
    ep: &db::episodes::Episode,
    clean_title: &str,
    jellyfin: Option<(&String, &String)>,
) -> bool {
    let _ = state.torrent.remove(ep.id, false).await;

    if let Some(path) = &ep.item_path {
        if let Err(e) = tokio::fs::remove_file(path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                state.activity.error(tr!("Erro ao apagar arquivo de \"{clean_title}\": {e}", "Failed to delete file of \"{clean_title}\": {e}"));
                return false;
            }
        }
    }

    if let (Some((url, api_key)), Some(item_id)) = (jellyfin, &ep.jellyfin_item_id) {
        if let Err(e) = jellyfin::delete_item(&state.http, url, api_key, item_id).await {
            state.activity.error(tr!("Erro ao remover \"{clean_title}\" do Jellyfin: {e}", "Failed to remove \"{clean_title}\" from Jellyfin: {e}"));
        }
    }

    if let Err(e) = db::episodes::mark_deleted(&state.db, ep.id).await {
        state.activity.error(format!("Erro ao marcar \"{clean_title}\" como removido: {e}"));
        return false;
    }
    true
}

pub async fn repair_missing_item_paths(state: &AppState) {
    let Ok(episodes) = db::episodes::list_by_status(&state.db, "available").await else { return };
    let tag = regex::Regex::new(r"(?i)S(\d{1,2})E(\d{1,3})").unwrap();
    for ep in episodes.iter().filter(|e| e.item_path.is_none()) {
        let (Some(name), Some(folder)) = (ep.name.as_deref(), ep.save_path.as_deref()) else { continue };
        let Some(want) = tag.captures(name).map(|c| (c[1].parse::<u32>().ok(), c[2].parse::<u32>().ok())) else {
            continue;
        };
        let Ok(entries) = std::fs::read_dir(folder) else { continue };
        let found = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                        ["mkv", "mp4", "avi", "webm"].contains(&x.to_ascii_lowercase().as_str())
                    })
            })
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| tag.captures(n))
                    .map(|c| (c[1].parse::<u32>().ok(), c[2].parse::<u32>().ok()))
                    == Some(want)
            })
            .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0));
        if let Some(path) = found {
            let _ = db::episodes::set_item_path(&state.db, ep.id, &path.to_string_lossy()).await;
        }
    }
}

pub async fn reconcile_missing_files(state: &AppState, episodes: &mut [db::episodes::Episode]) {
    for ep in episodes.iter_mut().filter(|e| e.status == "available") {
        let Some(path) = ep.item_path.as_deref() else { continue };
        let file = std::path::Path::new(path);
        if file.exists()
            || file.with_extension("torii-old.mkv").exists()
            || file.with_extension("torii-tmp.mkv").exists()
        {
            continue;
        }
        let _ = state.torrent.remove(ep.id, false).await;
        if db::episodes::mark_deleted(&state.db, ep.id).await.is_ok() {
            let name = ep.name.clone().unwrap_or_else(|| tr!("episódio #{}", "episode #{}", ep.id));
            state.activity.info(tr!(
                "Arquivo apagado fora do Torii, episódio marcado como removido: {name}",
                "File deleted outside Torii, episode marked as removed: {name}"
            ));
            ep.status = "deleted".to_string();
            ep.deleted_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }
}

/// Deleted this long after being watched (Torii's player or Jellyfin).
const WATCHED_DELETE_DELAY: chrono::Duration = chrono::Duration::minutes(30);
const WATCHED_CHECK_EVERY: Duration = Duration::from_secs(5 * 60);

/// Episodes finished in Jellyfin count as watched in Torii: by the chosen user, or by any
/// user when none is chosen. Other users' watching is ignored.
pub async fn sync_jellyfin_watched(state: &AppState) {
    let Ok(settings) = db::settings::get_all(&state.db).await else { return };
    let (true, Some(url), Some(key)) = (
        settings.get("jellyfin_mode").map(String::as_str) == Some("1"),
        settings.get("jellyfin_url").filter(|s| !s.is_empty()),
        settings.get("jellyfin_api_key").filter(|s| !s.is_empty()),
    ) else {
        return;
    };
    let Ok(episodes) = db::episodes::list_by_status(&state.db, "available").await else { return };
    let pending: Vec<(i64, String)> = episodes
        .into_iter()
        .filter(|e| e.watched_at.is_none())
        .filter_map(|e| e.jellyfin_item_id.map(|item| (e.id, item)))
        .collect();
    if pending.is_empty() {
        return;
    }
    let users: Vec<String> = match settings.get("jellyfin_user_id").filter(|s| !s.is_empty()) {
        Some(user) => vec![user.clone()],
        None => match jellyfin::list_users(&state.http, url, key).await {
            Ok(users) => users.into_iter().map(|u| u.id).collect(),
            Err(_) => return,
        },
    };
    let ids: Vec<String> = pending.iter().map(|(_, item)| item.clone()).collect();
    let mut played = Vec::new();
    for user in &users {
        match jellyfin::played_items(&state.http, url, key, user, &ids).await {
            Ok(p) => played.extend(p),
            Err(e) => {
                state.activity.error(tr!("Erro ao ler o que foi assistido no Jellyfin: {e}", "Failed to read watched items from Jellyfin: {e}"));
                return;
            }
        }
    }
    for (item, last_played) in played {
        let Some((episode_id, _)) = pending.iter().find(|(_, i)| *i == item) else { continue };
        let when = last_played
            .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339();
        let _ = sqlx::query("UPDATE episodes SET watched_at = ? WHERE id = ? AND watched_at IS NULL")
            .bind(&when)
            .bind(episode_id)
            .execute(&state.db)
            .await;
    }
}

/// Watched-state sync and cleanup run more often than searches, so "delete 30 minutes
/// after watching" holds regardless of the search interval.
pub fn spawn_watched_cleanup_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(WATCHED_CHECK_EVERY).await;
            let state = app.state::<AppState>();
            sync_jellyfin_watched(&state).await;
            cleanup_once(&state, currently_open_media(&app)).await;
        }
    });
}

pub async fn cleanup_once(state: &AppState, playing_path: Option<String>) {
    if let Ok(mut available) = db::episodes::list_available(&state.db).await {
        reconcile_missing_files(state, &mut available).await;
    }
    cleanup_watched_streams(state, playing_path.as_deref()).await;
    let settings = match db::settings::get_all(&state.db).await {
        Ok(s) => s,
        Err(_) => return,
    };
    let global_default: Option<i64> = settings
        .get("default_delete_after_days")
        .and_then(|v| v.parse().ok());
    let delete_after_watched = settings.get("delete_after_watched").map(String::as_str) == Some("1");
    let jellyfin_mode = settings.get("jellyfin_mode").map(String::as_str) == Some("1");
    let jellyfin_url = settings.get("jellyfin_url").filter(|s| !s.is_empty());
    let jellyfin_api_key = settings.get("jellyfin_api_key").filter(|s| !s.is_empty());
    let jellyfin = match (jellyfin_mode, jellyfin_url, jellyfin_api_key) {
        (true, Some(url), Some(key)) => Some((url, key)),
        _ => None,
    };

    let available = match db::episodes::list_by_status(&state.db, "available").await {
        Ok(e) => e,
        Err(_) => return,
    };
    let now = chrono::Utc::now();
    let parse = |s: Option<&str>| {
        s.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
    };

    for ep in available {
        if playing_path.is_some() && ep.item_path == playing_path
            || playing_path.as_deref().and_then(crate::stream_server::episode_of_url) == Some(ep.id)
        {
            continue;
        }
        let Ok(watch) = db::watches::get(&state.db, ep.watch_id).await else {
            continue;
        };

        let retention = watch.delete_after_days.or(global_default).filter(|d| *d > 0);
        let expired_days = match (retention, parse(ep.available_at.as_deref())) {
            (Some(days), Some(available_at)) if (now - available_at).num_days() >= days => Some(days),
            _ => None,
        };
        let watched_long_ago = parse(ep.watched_at.as_deref()).is_some_and(|watched_at| {
            watch.streaming
                || (watch.delete_after_watched.unwrap_or(delete_after_watched)
                    && now - watched_at >= WATCHED_DELETE_DELAY)
        });
        if expired_days.is_none() && !watched_long_ago {
            continue;
        }

        let title = ep.name.clone().unwrap_or_else(|| tr!("episódio #{}", "episode #{}", ep.id));
        let clean_title = notify_episode_title(&watch.title, &title);
        if remove_episode(state, &ep, &clean_title, jellyfin).await {
            match expired_days {
                Some(days) => state.activity.info(tr!("Removido por retenção ({days}d): {clean_title}", "Removed by retention ({days}d): {clean_title}")),
                None => state.activity.info(tr!("Removido depois de assistido: {clean_title}", "Removed after watching: {clean_title}")),
            }
        }
    }
}

pub async fn cleanup_watched_streams(state: &AppState, playing: Option<&str>) {
    let playing_episode = playing.and_then(crate::stream_server::episode_of_url);
    let mut candidates = Vec::new();
    for status in ["downloading", "found", "ready", "available"] {
        if let Ok(mut eps) = db::episodes::list_by_status(&state.db, status).await {
            candidates.append(&mut eps);
        }
    }
    for ep in candidates {
        if ep.watched_at.is_none() || playing_episode == Some(ep.id) || (playing.is_some() && ep.item_path.as_deref() == playing) {
            continue;
        }
        let Ok(watch) = db::watches::get(&state.db, ep.watch_id).await else { continue };
        if !watch.streaming {
            continue;
        }
        let _ = state.torrent.remove(ep.id, true).await;
        if let Some(path) = &ep.item_path {
            let _ = tokio::fs::remove_file(path).await;
        }
        if db::episodes::mark_deleted(&state.db, ep.id).await.is_ok() {
            let title = ep.name.clone().unwrap_or_else(|| tr!("episódio #{}", "episode #{}", ep.id));
            let clean_title = notify_episode_title(&watch.title, &title);
            state.activity.info(tr!("Removido depois de assistido: {clean_title}", "Removed after watching: {clean_title}"));
        }
    }
}

pub async fn backfill_placeholder_episodes(app: AppHandle) {
    let state = app.state::<AppState>();

    if let Ok(unnumbered) = db::episodes::list_missing_episode_number(&state.db).await {
        for ep in unnumbered {
            if let Some(n) = ep.name.as_deref().and_then(nyaa::extract_episode_number) {
                let _ = db::episodes::set_episode_number(&state.db, ep.id, n as i64).await;
            }
        }
    }

    let Ok(watches) = db::watches::list(&state.db).await else {
        return;
    };
    for watch in watches {
        let Some(total) = watch.episodes.filter(|n| *n > 0) else {
            continue;
        };
        for ep in 1..=total {
            let _ = db::episodes::create_placeholder(&state.db, watch.id, &watch.folder, ep).await;
        }
    }
}

pub async fn backfill_series(app: AppHandle) {
    let state = app.state::<AppState>();
    let Ok(watches) = db::watches::list(&state.db).await else {
        return;
    };
    for watch in watches.into_iter().filter(|w| w.series_anilist_id.is_none()) {
        let Some(anilist_id) = watch.anilist_id else { continue };
        if let Some((series_id, series_title)) =
            crate::commands::watches::resolve_series(&state.http, anilist_id).await
        {
            let _ = db::watches::set_series(&state.db, watch.id, series_id, &series_title).await;
        }
    }
}

pub async fn poll_watch_by_id(app: &AppHandle, watch_id: i64) {
    let state = app.state::<AppState>();
    if let Ok(watch) = db::watches::get(&state.db, watch_id).await {
        poll_watch(app, &state, &watch).await;
    }
}

fn currently_open_media(app: &AppHandle) -> Option<String> {
    use crate::player::ffi::VlcState;
    let player = app.state::<crate::player::PlayerState>();
    let state = player.engine.lock().unwrap().as_ref().map(|e| e.snapshot().state)?;
    if !matches!(state, VlcState::Playing | VlcState::Paused | VlcState::Buffering | VlcState::Opening) {
        return None;
    }
    let source = player.now_playing.lock().unwrap().source.clone();
    (!source.is_empty()).then_some(source)
}

pub fn spawn_background_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            {
                let state = app.state::<AppState>();
                poll_once(&app, &state).await;
                let playing = currently_open_media(&app);
                cleanup_once(&state, playing.clone()).await;
                crate::postprocess::spawn_pending(&app, playing);
                crate::reminders::run_once(&app, &state).await;
            }

            let interval_minutes: u64 = {
                let state = app.state::<AppState>();
                db::settings::get_all(&state.db)
                    .await
                    .ok()
                    .and_then(|s| s.get("poll_interval_minutes").cloned())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30)
            };
            let secs = (interval_minutes * 60).max(60);
            tokio::time::sleep(Duration::from_secs(secs)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_episode_title_strips_release_junk() {
        assert_eq!(
            notify_episode_title(
                "Re:ZERO -Starting Life in Another World- Season 4",
                "[ToonsHub] ReZERO -Starting Life in Another World- S04E01 1080p CR WEB-DL MULTi AAC2.0 H.264 (Re:Zero kara Hajimeru Isekai Seikatsu, Multi-Audio, Multi-Subs)",
            ),
            "Re:ZERO -Starting Life in Another World- — Episódio 1"
        );
    }

    #[test]
    fn notify_episode_title_falls_back_to_base_without_episode_number() {
        assert_eq!(notify_episode_title("Show Season 2", "Some batch release with no episode tag"), "Show");
    }
}
