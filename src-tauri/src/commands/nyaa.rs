use crate::{error::AppError, sources::nyaa, state::AppState};
use tauri::State;

#[tauri::command]
pub async fn nyaa_search(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<nyaa::NyaaCandidate>, AppError> {
    state.activity.info(tr!("Procurando episódios de \"{query}\"...", "Searching episodes of \"{query}\"..."));
    let result = nyaa::search(&state.http, &query).await;
    match &result {
        Ok(items) if items.is_empty() => {
            state.activity.info(tr!("Nada encontrado ainda pra \"{query}\"", "Nothing found yet for \"{query}\""))
        }
        Ok(items) => state
            .activity
            .info(tr!("Encontrado {} episódio(s) de \"{query}\"", "Found {} episode(s) of \"{query}\"", items.len())),
        Err(e) => state.activity.error(format!("Falha ao procurar \"{query}\": {e}")),
    }
    result.map_err(AppError::Fetch)
}

#[tauri::command]
pub async fn nyaa_available_languages(
    state: State<'_, AppState>,
    query: String,
) -> Result<nyaa::AvailableLanguages, AppError> {
    nyaa::list_available_languages(&state.http, &query)
        .await
        .map_err(AppError::Fetch)
}
