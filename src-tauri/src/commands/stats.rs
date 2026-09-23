use crate::{db, error::AppError, state::AppState};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Serialize)]
pub struct StorageStats {
    pub used_bytes: u64,
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            total += dir_size(&entry.path());
        } else {
            total += meta.len();
        }
    }
    total
}

#[tauri::command]
pub async fn get_storage_stats(state: State<'_, AppState>) -> Result<StorageStats, AppError> {
    let settings = db::settings::get_all(&state.db).await?;
    let root = settings.get("library_root").cloned().unwrap_or_default();

    let used_bytes = tauri::async_runtime::spawn_blocking(move || {
        if root.is_empty() {
            0
        } else {
            dir_size(&PathBuf::from(&root))
        }
    })
    .await
    .unwrap_or(0);

    Ok(StorageStats { used_bytes })
}
