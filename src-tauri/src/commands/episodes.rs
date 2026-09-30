use crate::{db, db::episode_sources::EpisodeSource, db::episodes::Episode, engine, error::AppError, state::AppState};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn list_recent_episodes(state: State<'_, AppState>) -> Result<Vec<Episode>, AppError> {
    Ok(db::episodes::list_recent(&state.db, 50).await?)
}

#[tauri::command]
pub async fn list_available_episodes(state: State<'_, AppState>) -> Result<Vec<Episode>, AppError> {
    let mut episodes = db::episodes::list_available(&state.db).await?;
    engine::reconcile_missing_files(&state, &mut episodes).await;
    Ok(episodes.into_iter().filter(|e| e.status == "available").collect())
}

#[tauri::command]
pub async fn list_watch_episodes(
    state: State<'_, AppState>,
    watch_id: i64,
) -> Result<Vec<Episode>, AppError> {
    let mut episodes = db::episodes::list_for_watch(&state.db, watch_id).await?;
    engine::reconcile_missing_files(&state, &mut episodes).await;
    Ok(episodes)
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

#[tauri::command]
pub async fn delete_episode(state: State<'_, AppState>, episode_id: i64) -> Result<(), AppError> {
    engine::delete_episode(&state, episode_id).await
}

#[tauri::command]
pub async fn force_check_episode(
    app: AppHandle,
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<bool, AppError> {
    engine::force_download_episode(&app, &state, episode_id, true).await
}

#[tauri::command]
pub async fn episode_stream_url(app: AppHandle, state: State<'_, AppState>, episode_id: i64) -> Result<String, AppError> {
    let server = app
        .try_state::<crate::stream_server::StreamServer>()
        .ok_or_else(|| AppError::Fetch("stream server unavailable".into()))?;
    let (_, _, _, name) = state
        .torrent
        .stream_target(episode_id)
        .await
        .ok_or_else(|| AppError::Fetch(tr!("esse episódio não está baixando", "this episode isn't downloading")))?;
    let extension = std::path::Path::new(&name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv")
        .to_ascii_lowercase();
    Ok(server.url(episode_id, &extension))
}

#[tauri::command]
pub async fn episode_stream_start(
    app: AppHandle,
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<String, AppError> {
    crate::engine::start_stream(&app, &state, episode_id).await?;
    episode_stream_url(app, state, episode_id).await
}

#[tauri::command]
pub async fn episode_prefetch_next(app: AppHandle, watch_id: i64, episode_number: i64) -> Result<(), AppError> {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        crate::engine::prefetch_next(&app, &state, watch_id, episode_number).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn download_missing_episodes(
    app: AppHandle,
    state: State<'_, AppState>,
    watch_id: i64,
) -> Result<usize, AppError> {
    let watch = db::watches::get(&state.db, watch_id).await?;
    let ids: Vec<i64> = db::episodes::list_for_watch(&state.db, watch_id)
        .await?
        .into_iter()
        .filter(|e| matches!(e.status.as_str(), "pending" | "deleted" | "error"))
        .filter(|e| {
            e.episode_number.is_none_or(|n| {
                watch.episode_start.is_none_or(|s| n >= s) && watch.episode_end.is_none_or(|end| n <= end)
            })
        })
        .map(|e| e.id)
        .collect();
    let count = ids.len();
    if count > 0 {
        state.activity.info(tr!(
            "Buscando {count} episódio(s) de \"{}\"...",
            "Searching {count} episode(s) of \"{}\"...",
            watch.title
        ));
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            for id in ids {
                let _ = engine::force_download_episode(&app, &state, id, false).await;
            }
        });
    }
    Ok(count)
}

#[tauri::command]
pub async fn list_episode_sources(
    state: State<'_, AppState>,
    episode_id: i64,
) -> Result<Vec<EpisodeSource>, AppError> {
    Ok(db::episode_sources::list(&state.db, episode_id).await?)
}

#[tauri::command]
pub async fn switch_episode_source(
    app: AppHandle,
    state: State<'_, AppState>,
    episode_id: i64,
    source_item_id: String,
) -> Result<(), AppError> {
    engine::switch_source(&app, &state, episode_id, &source_item_id).await?;
    Ok(())
}
