use crate::{ffmpeg, state::AppState};
use tauri::{AppHandle, Manager};

/// Estado do ffmpeg baixado sob demanda (Config mostra "baixando 42%" etc.).
#[tauri::command]
pub fn ffmpeg_status(app: AppHandle) -> ffmpeg::InstallStatus {
    ffmpeg::status(&app)
}

/// Dispara o download do ffmpeg em segundo plano (volta na hora) e, quando
/// terminar, já roda o pós-processamento pendente — chamado quando o
/// usuário liga uma das opções, pra não esperar o próximo ciclo de 30min.
#[tauri::command]
pub fn ffmpeg_install(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let http = app.state::<AppState>().http.clone();
        if ffmpeg::ensure_installed(&app, &http).await.is_ok() {
            crate::postprocess::spawn_pending(&app, None);
        }
    });
}

/// Sinopse traduzida pro idioma `target` ("pt"). Cai no texto original se a
/// tradução falhar (sem internet, limite do serviço etc.).
#[tauri::command]
pub async fn translate_text(
    state: tauri::State<'_, AppState>,
    text: String,
    target: String,
) -> Result<String, ()> {
    Ok(crate::sources::translate::translate(&state.http, &state.db, &text, &target).await)
}
