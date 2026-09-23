use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

/// Trechos de abertura/encerramento de 1 episódio, em ms — `None` num campo
/// = esse trecho não existe/não foi achado (não é erro, ver `aniskip.rs`).
#[derive(Debug, Clone, Default, Serialize, FromRow)]
pub struct SkipSegments {
    pub intro_start_ms: Option<i64>,
    pub intro_end_ms: Option<i64>,
    pub ending_start_ms: Option<i64>,
    pub ending_end_ms: Option<i64>,
    /// Resumo do episódio anterior.
    pub recap_start_ms: Option<i64>,
    pub recap_end_ms: Option<i64>,
}

pub async fn get_cached(
    pool: &SqlitePool,
    watch_id: i64,
    episode_number: i64,
) -> Result<Option<SkipSegments>, sqlx::Error> {
    sqlx::query_as::<_, SkipSegments>(
        "SELECT intro_start_ms, intro_end_ms, ending_start_ms, ending_end_ms, recap_start_ms, recap_end_ms \
         FROM skip_segments WHERE watch_id = ? AND episode_number = ?",
    )
    .bind(watch_id)
    .bind(episode_number)
    .fetch_optional(pool)
    .await
}

pub async fn upsert(
    pool: &SqlitePool,
    watch_id: i64,
    episode_number: i64,
    segments: &SkipSegments,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO skip_segments \
         (watch_id, episode_number, intro_start_ms, intro_end_ms, ending_start_ms, ending_end_ms, \
          recap_start_ms, recap_end_ms, fetched_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(watch_id, episode_number) DO UPDATE SET \
           intro_start_ms = excluded.intro_start_ms, \
           intro_end_ms = excluded.intro_end_ms, \
           ending_start_ms = excluded.ending_start_ms, \
           ending_end_ms = excluded.ending_end_ms, \
           recap_start_ms = excluded.recap_start_ms, \
           recap_end_ms = excluded.recap_end_ms, \
           fetched_at = excluded.fetched_at",
    )
    .bind(watch_id)
    .bind(episode_number)
    .bind(segments.intro_start_ms)
    .bind(segments.intro_end_ms)
    .bind(segments.ending_start_ms)
    .bind(segments.ending_end_ms)
    .bind(segments.recap_start_ms)
    .bind(segments.recap_end_ms)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}
