use crate::{db, error::AppError, i18n, state::AppState};
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<HashMap<String, String>, AppError> {
    Ok(db::settings::get_all(&state.db).await?)
}

#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    values: HashMap<String, String>,
) -> Result<(), AppError> {
    db::settings::update(&state.db, &values).await?;
    if values.get("detect_segments").map(String::as_str) == Some("1") {
        crate::intro_detect::spawn_pending(&app);
    }
    if let Some(language) = values.get("app_language") {
        i18n::apply_setting(language);
        if let Some(tray) = app.try_state::<i18n::TrayMenu>() {
            tray.refresh();
        }
    }
    Ok(())
}
