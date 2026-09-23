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

/// Anime (1ª temporada da franquia na AniList) a que essa temporada
/// pertence: (anilist_id da raiz, título do anime sem "Season N"). `None`
/// se a AniList não responder a tempo — aí vale o agrupamento por título.
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

    // Cria a linha de cada episódio da temporada já na Biblioteca (status
    // "pending"), antes de qualquer busca no Nyaa — sem isso o episódio só
    // aparece quando (e se) o poller acha um torrent, e problema de busca
    // vira "episódio sumido" pro usuário. TODOS os episódios, não só o
    // intervalo escolhido: fora do intervalo aparece "Não baixado" na
    // Biblioteca (o poller já não busca fora dele) em vez de sumir. Sem
    // contagem de episódio (anime ainda no ar sem total definido no
    // AniList) continua descoberta incrementalmente.
    if let Some(total) = created.episodes.filter(|n| *n > 0) {
        for ep in 1..=total {
            if let Err(e) = db::episodes::create_placeholder(&state.db, created.id, &created.folder, ep).await {
                state.activity.error(format!("Erro ao criar placeholder do episódio {ep}: {e}"));
            }
        }
    }

    let watch_id = created.id;
    tauri::async_runtime::spawn(async move {
        engine::poll_watch_by_id(&app, watch_id).await;
    });

    Ok(created)
}

#[tauri::command]
pub async fn delete_watch(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    Ok(db::watches::delete(&state.db, id).await?)
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
    state: State<'_, AppState>,
    id: i64,
    prefs: WatchPreferences,
) -> Result<(), AppError> {
    Ok(db::watches::set_preferences(&state.db, id, prefs).await?)
}
