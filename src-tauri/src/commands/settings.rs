use crate::{db, error::AppError, state::AppState};
use std::collections::HashMap;
use tauri::State;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<HashMap<String, String>, AppError> {
    Ok(db::settings::get_all(&state.db).await?)
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    values: HashMap<String, String>,
) -> Result<(), AppError> {
    db::settings::update(&state.db, &values).await?;
    Ok(())
}
