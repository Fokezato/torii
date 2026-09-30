use crate::{ffmpeg, state::AppState};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn ffmpeg_status(app: AppHandle) -> ffmpeg::InstallStatus {
    ffmpeg::status(&app)
}

#[tauri::command]
pub fn ffmpeg_install(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let http = app.state::<AppState>().http.clone();
        if ffmpeg::ensure_installed(&app, &http).await.is_ok() {
            crate::postprocess::spawn_pending(&app, None);
        }
    });
}

#[tauri::command]
pub async fn translate_text(
    state: tauri::State<'_, AppState>,
    text: String,
    target: String,
) -> Result<String, ()> {
    Ok(crate::sources::translate::translate(&state.http, &state.db, &text, &target).await)
}
