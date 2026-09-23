use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Serialize, FromRow)]
pub struct Watch {
    pub id: i64,
    pub title: String,
    pub query: String,
    pub anilist_id: Option<i64>,
    /// ID no MyAnimeList — resolvido sob demanda (não no create) via campo
    /// `idMal` da AniList, na 1ª vez que o player precisa buscar skip
    /// segments (ver `sources::aniskip`). `None` até então.
    pub mal_id: Option<i64>,
    pub cover_url: Option<String>,
    pub quality: String,
    pub audio_lang: Option<String>,
    pub sub_lang: Option<String>,
    pub folder: String,
    pub delete_after_days: Option<i64>,
    pub rating: Option<i64>,
    pub active: bool,
    pub notify_on_available: bool,
    pub status: Option<String>,
    pub list_status: String,
    pub episodes: Option<i64>,
    pub episode_start: Option<i64>,
    pub episode_end: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    /// "Reduzir resolução" desse anime: "720p" = ligado; NULL/"original" =
    /// desligado. A opção global em Config, quando ligada, vale por cima.
    pub max_resolution: Option<String>,
    /// "Remover áudios extras" desse anime — mesma regra da global.
    pub strip_audio: bool,
    /// Anime (1ª temporada da franquia na AniList) a que essa temporada
    /// pertence — chave de agrupamento na Biblioteca. NULL = ainda não
    /// resolvido (sem anilist_id, ou AniList fora do ar).
    pub series_anilist_id: Option<i64>,
    pub series_title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewWatch {
    pub title: String,
    pub query: String,
    pub anilist_id: Option<i64>,
    pub cover_url: Option<String>,
    pub quality: Option<String>,
    pub audio_lang: Option<String>,
    pub sub_lang: Option<String>,
    pub folder: Option<String>,
    pub delete_after_days: Option<i64>,
    pub active: Option<bool>,
    pub notify_on_available: Option<bool>,
    pub status: Option<String>,
    pub list_status: Option<String>,
    pub episodes: Option<i64>,
    pub episode_start: Option<i64>,
    pub episode_end: Option<i64>,
    pub max_resolution: Option<String>,
    pub strip_audio: Option<bool>,
    /// Preenchidos pelo backend (ver `commands::watches::create_watch`),
    /// não pelo front.
    #[serde(skip)]
    pub series_anilist_id: Option<i64>,
    #[serde(skip)]
    pub series_title: Option<String>,
}

fn sanitize_folder_name(name: &str) -> String {
    name.chars().filter(|c| !"<>:\"/\\|?*".contains(*c)).collect()
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Watch>, sqlx::Error> {
    sqlx::query_as::<_, Watch>("SELECT * FROM watches ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<Watch, sqlx::Error> {
    sqlx::query_as::<_, Watch>("SELECT * FROM watches WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn create(pool: &SqlitePool, library_root: &str, w: NewWatch) -> Result<Watch, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let folder = w.folder.unwrap_or_else(|| {
        // Pasta = nome do ANIME, não da temporada: o Jellyfin só agrupa como
        // a mesma série se todas as temporadas morarem na mesma pasta pai.
        // Nome do anime vem da 1ª temporada na AniList quando resolvido
        // (cobre temporada sem "Season N" no título); senão, tira o
        // "Season N" do título.
        let base_title = match &w.series_title {
            Some(series) => series.clone(),
            None => crate::sources::nyaa::split_season(&w.title).0,
        };
        // `Path::join` usa o separador certo do SO — um `format!("{}/{}", ...)`
        // aqui misturava "\" (do library_root do Windows) com "/" (literal),
        // e esse path quebrado ia parar em `episodes.save_path`, fazendo o
        // botão "Abrir pasta" falhar silenciosamente (bug real reportado).
        std::path::Path::new(library_root)
            .join(sanitize_folder_name(&base_title))
            .to_string_lossy()
            .to_string()
    });
    let quality = w.quality.unwrap_or_else(|| "1080p".to_string());
    let active = w.active.unwrap_or(true);
    let notify_on_available = w.notify_on_available.unwrap_or(true);
    let list_status = w.list_status.unwrap_or_else(|| "watching".to_string());

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO watches \
         (title, query, anilist_id, cover_url, quality, audio_lang, sub_lang, folder, delete_after_days, active, notify_on_available, status, list_status, episodes, episode_start, episode_end, max_resolution, strip_audio, series_anilist_id, series_title, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&w.title)
    .bind(&w.query)
    .bind(w.anilist_id)
    .bind(&w.cover_url)
    .bind(&quality)
    .bind(&w.audio_lang)
    .bind(&w.sub_lang)
    .bind(&folder)
    .bind(w.delete_after_days)
    .bind(active)
    .bind(notify_on_available)
    .bind(&w.status)
    .bind(&list_status)
    .bind(w.episodes)
    .bind(w.episode_start)
    .bind(w.episode_end)
    .bind(&w.max_resolution)
    .bind(w.strip_audio.unwrap_or(false))
    .bind(w.series_anilist_id)
    .bind(&w.series_title)
    .bind(&now)
    .bind(&now)
    .fetch_one(pool)
    .await?;

    get(pool, id).await
}

pub async fn set_series(pool: &SqlitePool, id: i64, series_anilist_id: i64, series_title: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE watches SET series_anilist_id = ?, series_title = ? WHERE id = ?")
        .bind(series_anilist_id)
        .bind(series_title)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_mal_id(pool: &SqlitePool, id: i64, mal_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE watches SET mal_id = ? WHERE id = ?")
        .bind(mal_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM watches WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn set_active(pool: &SqlitePool, id: i64, active: bool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE watches SET active = ?, updated_at = ? WHERE id = ?")
        .bind(active)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_rating(pool: &SqlitePool, id: i64, rating: Option<i64>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE watches SET rating = ?, updated_at = ? WHERE id = ?")
        .bind(rating)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_list_status(pool: &SqlitePool, id: i64, list_status: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE watches SET list_status = ?, updated_at = ? WHERE id = ?")
        .bind(list_status)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct WatchPreferences {
    pub quality: String,
    pub audio_lang: Option<String>,
    pub sub_lang: Option<String>,
    pub delete_after_days: Option<i64>,
    pub notify_on_available: bool,
    pub episode_start: Option<i64>,
    pub episode_end: Option<i64>,
    #[serde(default)]
    pub max_resolution: Option<String>,
    #[serde(default)]
    pub strip_audio: bool,
}

pub async fn set_preferences(
    pool: &SqlitePool,
    id: i64,
    prefs: WatchPreferences,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE watches SET quality = ?, audio_lang = ?, sub_lang = ?, delete_after_days = ?, \
         notify_on_available = ?, episode_start = ?, episode_end = ?, max_resolution = ?, \
         strip_audio = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&prefs.quality)
    .bind(&prefs.audio_lang)
    .bind(&prefs.sub_lang)
    .bind(prefs.delete_after_days)
    .bind(prefs.notify_on_available)
    .bind(prefs.episode_start)
    .bind(prefs.episode_end)
    .bind(&prefs.max_resolution)
    .bind(prefs.strip_audio)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
