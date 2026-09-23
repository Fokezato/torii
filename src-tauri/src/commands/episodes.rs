use crate::{db, db::episode_sources::EpisodeSource, db::episodes::Episode, engine, error::AppError, state::AppState};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn list_recent_episodes(state: State<'_, AppState>) -> Result<Vec<Episode>, AppError> {
    Ok(db::episodes::list_recent(&state.db, 50).await?)
}

#[tauri::command]
pub async fn list_watch_episodes(
    state: State<'_, AppState>,
    watch_id: i64,
) -> Result<Vec<Episode>, AppError> {
    Ok(db::episodes::list_for_watch(&state.db, watch_id).await?)
}

#[tauri::command]
pub async fn pause_episode_download(state: State<'_, AppState>, episode_id: i64) -> Result<(), AppError> {
    state.torrent.pause(episode_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn resume_episode_download(state: State<'_, AppState>, episode_id: i64) -> Result<(), AppError> {
    state.torrent.resume(episode_id).await?;
    Ok(())
}

/// Cancela um download incompleto. Volta o episódio pra "pending" em vez de
/// "deleted" — a linha é o placeholder permanente do episódio na Biblioteca
/// (ver `create_placeholder`), então cancelar não deve fazê-lo sumir, só
/// esquecer a tentativa e deixar procurável de novo no próximo poll.
#[tauri::command]
pub async fn cancel_episode_download(
    state: State<'_, AppState>,
    episode_id: i64,
    delete_files: bool,
) -> Result<(), AppError> {
    state.torrent.remove(episode_id, delete_files).await?;
    db::episodes::mark_pending(&state.db, episode_id).await?;
    Ok(())
}

/// Menu "..." > Forçar verificação na Biblioteca: busca só esse episódio no
/// Nyaa agora, ignorando o ciclo de poll normal — útil quando o poller
/// nunca chegou nele ainda, ou a última tentativa falhou e o usuário não
/// quer esperar o próximo ciclo. Devolve `true` se achou e iniciou
/// download, `false` se não achou candidato nenhum dessa vez.
#[tauri::command]
pub async fn force_check_episode(
    app: AppHandle,
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<bool, AppError> {
    let episode = db::episodes::get(&state.db, episode_id).await?;
    let watch = db::watches::get(&state.db, episode.watch_id).await?;
    let Some(episode_number) = episode
        .episode_number
        .or_else(|| episode.name.as_deref().and_then(crate::sources::nyaa::extract_episode_number).map(i64::from))
    else {
        return Err(AppError::Fetch(tr!("episódio sem número identificável", "episode has no recognizable number")));
    };

    let audio_langs = engine::split_langs(&watch.audio_lang);
    let sub_langs = engine::split_langs(&watch.sub_lang);
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
    engine::start_download(
        &app,
        &state,
        episode_id,
        &watch.title,
        &candidate.title,
        &candidate.magnet,
        &watch.folder,
        watch.cover_url,
        true,
    )
    .await;
    Ok(true)
}

#[tauri::command]
pub async fn list_episode_sources(
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<Vec<EpisodeSource>, AppError> {
    Ok(db::episode_sources::list(&state.db, episode_id).await?)
}

/// Menu "..." > Trocar fonte na Biblioteca: cancela o torrent/arquivo atual
/// (se algum) e reinicia o download a partir da fonte alternativa escolhida.
#[tauri::command]
pub async fn switch_episode_source(
    app: AppHandle,
    state: State<'_, AppState>,
    episode_id: i64,
    source_item_id: String,
) -> Result<(), AppError> {
    let source = db::episode_sources::get(&state.db, episode_id, &source_item_id).await?;
    let _ = state.torrent.remove(episode_id, true).await;
    db::episodes::switch_source(&state.db, episode_id, &source.source_item_id, &source.title, &source.magnet_uri)
        .await?;
    db::episode_sources::set_active(&state.db, episode_id, &source_item_id).await?;

    let episode = db::episodes::get(&state.db, episode_id).await?;
    let watch = db::watches::get(&state.db, episode.watch_id).await?;
    engine::start_download(
        &app,
        &state,
        episode_id,
        &watch.title,
        &source.title,
        &source.magnet_uri,
        &watch.folder,
        watch.cover_url,
        true,
    )
    .await;
    Ok(())
}
