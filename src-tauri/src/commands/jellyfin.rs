use crate::{db, error::AppError, jellyfin, state::AppState};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct JellyfinTestResult {
    pub server_name: String,
    pub version: String,
}

#[tauri::command]
pub async fn test_jellyfin_connection(state: State<'_, AppState>) -> Result<JellyfinTestResult, AppError> {
    let settings = db::settings::get_all(&state.db).await?;
    let url = settings
        .get("jellyfin_url")
        .cloned()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Fetch(tr!("configura a URL do Jellyfin antes de testar", "set the Jellyfin URL before testing")))?;
    let api_key = settings
        .get("jellyfin_api_key")
        .cloned()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Fetch(tr!("configura a API key do Jellyfin antes de testar", "set the Jellyfin API key before testing")))?;

    let info = jellyfin::test_connection(&state.http, &url, &api_key)
        .await
        .map_err(AppError::Fetch)?;

    Ok(JellyfinTestResult {
        server_name: info.server_name,
        version: info.version,
    })
}
