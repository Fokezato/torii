use crate::{error::AppError, sources::nyaa, state::AppState};
use tauri::State;

#[tauri::command]
pub async fn nyaa_available_languages(
    state: State<'_, AppState>,
    query: String,
) -> Result<nyaa::AvailableLanguages, AppError> {
    nyaa::list_available_languages(&state.http, &query)
        .await
        .map_err(AppError::Fetch)
}
