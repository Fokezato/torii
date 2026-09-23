use crate::{
    error::AppError,
    sources::{anilist, news},
    state::AppState,
};
use tauri::State;

#[tauri::command]
pub async fn browse_season(
    state: State<'_, AppState>,
    season: Option<String>,
    year: Option<i32>,
) -> Result<Vec<anilist::AnimeSummary>, AppError> {
    anilist::browse_season(&state.http, season.as_deref(), year)
        .await
        .map_err(AppError::Fetch)
}

#[tauri::command]
pub async fn browse_trending(
    state: State<'_, AppState>,
) -> Result<Vec<anilist::AnimeSummary>, AppError> {
    anilist::browse_trending(&state.http).await.map_err(AppError::Fetch)
}

#[tauri::command]
pub async fn anilist_search(
    state: State<'_, AppState>,
    q: String,
) -> Result<Vec<anilist::AnimeSummary>, AppError> {
    anilist::search(&state.http, &q).await.map_err(AppError::Fetch)
}

/// Todas as temporadas (TV) do anime, em ordem de lançamento — abas e
/// "Baixar outra temporada" na página do anime na Biblioteca.
#[tauri::command]
pub async fn anime_seasons(
    state: State<'_, AppState>,
    anilist_id: i32,
) -> Result<Vec<anilist::AnimeSummary>, AppError> {
    anilist::franchise_seasons(&state.http, anilist_id).await.map_err(AppError::Fetch)
}

#[tauri::command]
pub async fn get_anime_news(state: State<'_, AppState>) -> Result<Vec<news::NewsItem>, AppError> {
    news::fetch(&state.http).await.map_err(AppError::Fetch)
}

#[tauri::command]
pub async fn get_schedule(
    state: State<'_, AppState>,
    ids: Vec<i32>,
) -> Result<Vec<anilist::AnimeSummary>, AppError> {
    anilist::by_ids(&state.http, &ids).await.map_err(AppError::Fetch)
}
