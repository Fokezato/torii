use crate::{
    db::{
        self,
        watches::{NewWatch, Watch, WatchPreferences},
    },
    engine,
    error::AppError,
    state::AppState,
};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn list_watches(state: State<'_, AppState>) -> Result<Vec<Watch>, AppError> {
    Ok(db::watches::list(&state.db).await?)
}

pub async fn resolve_series(http: &reqwest::Client, anilist_id: i64) -> Option<(i64, String)> {
    let seasons = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        crate::sources::anilist::franchise_seasons(http, anilist_id as i32),
    )
    .await
    .ok()?
    .ok()?;
    let root = seasons.first()?;
    Some((root.anilist_id as i64, crate::sources::nyaa::split_season(&root.title).0))
}

#[tauri::command]
pub async fn create_watch(
    state: State<'_, AppState>,
    app: AppHandle,
    mut watch: NewWatch,
) -> Result<Watch, AppError> {
    if let Some(anilist_id) = watch.anilist_id {
        if let Some((series_id, series_title)) = resolve_series(&state.http, anilist_id).await {
            watch.series_anilist_id = Some(series_id);
            watch.series_title = Some(series_title);
        }
    }
    let settings = db::settings::get_all(&state.db).await?;
    let library_root = settings.get("library_root").cloned().unwrap_or_default();
    let created = db::watches::create(&state.db, &library_root, watch).await?;

    if let Some(total) = created.episodes.filter(|n| *n > 0) {
        for ep in 1..=total {
            if let Err(e) = db::episodes::create_placeholder(&state.db, created.id, &created.folder, ep).await {
                state.activity.error(tr!("Erro ao criar placeholder do episódio {ep}: {e}", "Failed to create placeholder for episode {ep}: {e}"));
            }
        }
    }

    let watch_id = created.id;
    tauri::async_runtime::spawn(async move {
        engine::poll_watch_by_id(&app, watch_id).await;
    });

    Ok(created)
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RemoveMode {
    Everything,
    KeepFiles,
    FilesOnly,
}

#[tauri::command]
pub async fn remove_watch(state: State<'_, AppState>, id: i64, mode: RemoveMode) -> Result<u32, AppError> {
    let watch = db::watches::get(&state.db, id).await?;
    let episodes = db::episodes::list_for_watch(&state.db, id).await?;
    let delete_files = mode != RemoveMode::KeepFiles;

    let mut failed = 0u32;
    for ep in &episodes {
        let _ = state.torrent.remove(ep.id, delete_files).await;
        if delete_files && ep.status == "available" {
            if let Some(path) = ep.item_path.as_deref() {
                match std::fs::remove_file(path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => failed += 1,
                }
            }
        }
    }
    if delete_files {
        let _ = std::fs::remove_dir(&watch.folder);
    }

    if mode == RemoveMode::FilesOnly {
        for ep in episodes
            .iter()
            .filter(|e| matches!(e.status.as_str(), "available" | "downloading" | "found" | "ready" | "error"))
        {
            db::episodes::mark_deleted(&state.db, ep.id).await?;
        }
        state.activity.info(tr!(
            "Arquivos apagados: {}",
            "Files deleted: {}",
            watch.title
        ));
    } else {
        db::watches::delete(&state.db, id).await?;
        state.activity.info(tr!(
            "Removido da biblioteca: {}",
            "Removed from library: {}",
            watch.title
        ));
    }
    Ok(failed)
}

#[tauri::command]
pub async fn set_watch_active(
    state: State<'_, AppState>,
    id: i64,
    active: bool,
) -> Result<(), AppError> {
    Ok(db::watches::set_active(&state.db, id, active).await?)
}

#[tauri::command]
pub async fn set_watch_rating(
    state: State<'_, AppState>,
    id: i64,
    rating: Option<i64>,
) -> Result<(), AppError> {
    Ok(db::watches::set_rating(&state.db, id, rating).await?)
}

#[tauri::command]
pub async fn set_watch_list_status(
    state: State<'_, AppState>,
    id: i64,
    list_status: String,
) -> Result<(), AppError> {
    Ok(db::watches::set_list_status(&state.db, id, &list_status).await?)
}

#[tauri::command]
pub async fn set_watch_preferences(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    prefs: WatchPreferences,
) -> Result<(), AppError> {
    db::watches::set_preferences(&state.db, id, prefs).await?;
    db::seen_items::forget_rejected(&state.db, id).await?;
    tauri::async_runtime::spawn(async move {
        engine::poll_watch_by_id(&app, id).await;
    });
    Ok(())
}
