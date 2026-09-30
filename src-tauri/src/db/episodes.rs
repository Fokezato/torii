use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Serialize, FromRow)]
pub struct Episode {
    pub id: i64,
    pub watch_id: i64,
    pub source_item_id: Option<String>,
    pub name: Option<String>,
    pub magnet_uri: Option<String>,
    pub info_hash: Option<String>,
    pub save_path: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub jellyfin_item_id: Option<String>,
    pub item_path: Option<String>,
    pub added_at: String,
    pub available_at: Option<String>,
    pub deleted_at: Option<String>,
    pub episode_number: Option<i64>,
    pub watch_position_ms: Option<i64>,
    pub watched_at: Option<String>,
    pub audio_processed_at: Option<String>,
    pub video_processed_at: Option<String>,
    pub watch_progress_at: Option<String>,
    #[sqlx(default)]
    pub container_fixed_at: Option<String>,
    pub file_index: Option<i64>,
}

pub struct NewEpisode<'a> {
    pub watch_id: i64,
    pub source_item_id: &'a str,
    pub name: &'a str,
    pub magnet_uri: &'a str,
    pub save_path: &'a str,
    pub status: &'a str,
    pub episode_number: Option<i64>,
}

pub async fn add(pool: &SqlitePool, e: NewEpisode<'_>) -> Result<Episode, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO episodes (watch_id, source_item_id, name, magnet_uri, save_path, status, episode_number, added_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(e.watch_id)
    .bind(e.source_item_id)
    .bind(e.name)
    .bind(e.magnet_uri)
    .bind(e.save_path)
    .bind(e.status)
    .bind(e.episode_number)
    .bind(&now)
    .execute(pool)
    .await?;

    if let Some(episode_number) = e.episode_number {
        if let Some(row) = sqlx::query_as::<_, Episode>(
            "SELECT * FROM episodes WHERE watch_id = ? AND episode_number = ?",
        )
        .bind(e.watch_id)
        .bind(episode_number)
        .fetch_optional(pool)
        .await?
        {
            return Ok(row);
        }
    }

    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE watch_id = ? AND source_item_id = ?")
        .bind(e.watch_id)
        .bind(e.source_item_id)
        .fetch_one(pool)
        .await
}

pub async fn create_placeholder(
    pool: &SqlitePool,
    watch_id: i64,
    save_path: &str,
    episode_number: i64,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO episodes (watch_id, save_path, status, episode_number, added_at) \
         VALUES (?, ?, 'pending', ?, ?)",
    )
    .bind(watch_id)
    .bind(save_path)
    .bind(episode_number)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_missing_episode_number(pool: &SqlitePool) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>(
        "SELECT * FROM episodes WHERE episode_number IS NULL AND name IS NOT NULL AND status != 'deleted'",
    )
    .fetch_all(pool)
    .await
}

pub async fn set_episode_number(pool: &SqlitePool, id: i64, episode_number: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET episode_number = ? WHERE id = ?")
        .bind(episode_number)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_pending(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE episodes SET status = 'pending', source_item_id = NULL, name = NULL, \
         magnet_uri = NULL, info_hash = NULL, item_path = NULL, jellyfin_item_id = NULL, \
         error_message = NULL, available_at = NULL, deleted_at = NULL \
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_recent(pool: &SqlitePool, limit: i64) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>(
        "SELECT * FROM episodes WHERE status IN ('found', 'downloading', 'error', 'available') \
         ORDER BY CASE WHEN status IN ('found', 'downloading') THEN 0 ELSE 1 END, \
                  COALESCE(available_at, added_at) DESC \
         LIMIT ?",
    )
        .bind(limit)
        .fetch_all(pool)
        .await
}

pub async fn list_available(pool: &SqlitePool) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE status = 'available'")
        .fetch_all(pool)
        .await
}

pub async fn list_for_watch(pool: &SqlitePool, watch_id: i64) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE watch_id = ? ORDER BY added_at DESC")
        .bind(watch_id)
        .fetch_all(pool)
        .await
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<Episode, sqlx::Error> {
    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn list_by_status(pool: &SqlitePool, status: &str) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE status = ? ORDER BY added_at")
        .bind(status)
        .fetch_all(pool)
        .await
}

pub async fn set_item_path(pool: &SqlitePool, id: i64, item_path: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET item_path = ? WHERE id = ?")
        .bind(item_path)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_jellyfin_item_id(pool: &SqlitePool, id: i64, jellyfin_item_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET jellyfin_item_id = ? WHERE id = ?")
        .bind(jellyfin_item_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_downloading(pool: &SqlitePool, id: i64, info_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET status = 'downloading', info_hash = ?, error_message = NULL WHERE id = ?")
        .bind(info_hash)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_file_index(pool: &SqlitePool, id: i64, file_index: Option<i64>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET file_index = ? WHERE id = ?")
        .bind(file_index)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_ready(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET status = 'ready', info_hash = NULL, error_message = NULL WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_available(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE episodes SET status = 'available', available_at = COALESCE(available_at, ?) WHERE id = ?",
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_error(pool: &SqlitePool, id: i64, message: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET status = 'error', error_message = ? WHERE id = ?")
        .bind(message)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn switch_source(
    pool: &SqlitePool,
    id: i64,
    source_item_id: &str,
    name: &str,
    magnet_uri: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE episodes SET source_item_id = ?, name = ?, magnet_uri = ?, status = 'found', \
         info_hash = NULL, item_path = NULL, jellyfin_item_id = NULL, error_message = NULL, \
         available_at = NULL, deleted_at = NULL, file_index = NULL \
         WHERE id = ?",
    )
    .bind(source_item_id)
    .bind(name)
    .bind(magnet_uri)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn save_progress(
    pool: &SqlitePool,
    watch_id: i64,
    episode_number: i64,
    position_ms: i64,
    watched: bool,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE episodes SET watch_position_ms = ?, watch_progress_at = ?, \
         watched_at = CASE WHEN ? THEN COALESCE(watched_at, ?) ELSE watched_at END \
         WHERE watch_id = ? AND episode_number = ? AND status IN ('available', 'downloading')",
    )
    .bind(position_ms)
    .bind(&now)
    .bind(watched)
    .bind(&now)
    .bind(watch_id)
    .bind(episode_number)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_postprocess_pending(pool: &SqlitePool) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>(
        "SELECT * FROM episodes WHERE status = 'available' AND item_path IS NOT NULL \
         AND (audio_processed_at IS NULL OR video_processed_at IS NULL) ORDER BY available_at",
    )
    .fetch_all(pool)
    .await
}

pub async fn list_container_pending(pool: &SqlitePool) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>(
        "SELECT * FROM episodes WHERE status = 'available' AND item_path IS NOT NULL \
         AND lower(item_path) LIKE '%.mkv' AND container_fixed_at IS NULL ORDER BY available_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn mark_container_fixed(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET container_fixed_at = ? WHERE id = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_video_processed(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET video_processed_at = ? WHERE id = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_audio_processed(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET audio_processed_at = ? WHERE id = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_deleted(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE episodes SET status = 'deleted', deleted_at = ? WHERE id = ?")
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
