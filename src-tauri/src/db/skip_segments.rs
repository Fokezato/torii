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
    /// Abertura/encerramento "misto" do AniSkip (créditos por cima de cenas
    /// do episódio): o player mostra o botão mas não pula sozinho.
    #[sqlx(default)]
    pub intro_mixed: bool,
    #[sqlx(default)]
    pub ending_mixed: bool,
}

impl SkipSegments {
    /// Abertura e encerramento achados — não vale a pena perguntar de novo.
    pub fn is_complete(&self) -> bool {
        self.intro_start_ms.is_some() && self.ending_start_ms.is_some()
    }

    /// Completa o que faltar com os trechos detectados localmente.
    pub fn fill_from(&mut self, detected: &SkipSegments) {
        if self.intro_start_ms.is_none() {
            self.intro_start_ms = detected.intro_start_ms;
            self.intro_end_ms = detected.intro_end_ms;
        }
        if self.ending_start_ms.is_none() {
            self.ending_start_ms = detected.ending_start_ms;
            self.ending_end_ms = detected.ending_end_ms;
        }
    }
}

#[derive(FromRow)]
struct CachedRow {
    #[sqlx(flatten)]
    segments: SkipSegments,
    fetched_at: String,
}

/// Trechos em cache + quando foram buscados.
pub async fn get_cached(
    pool: &SqlitePool,
    watch_id: i64,
    episode_number: i64,
) -> Result<Option<(SkipSegments, chrono::DateTime<chrono::Utc>)>, sqlx::Error> {
    let row = sqlx::query_as::<_, CachedRow>(
        "SELECT intro_start_ms, intro_end_ms, ending_start_ms, ending_end_ms, recap_start_ms, recap_end_ms, \
         intro_mixed, ending_mixed, fetched_at \
         FROM skip_segments WHERE watch_id = ? AND episode_number = ?",
    )
    .bind(watch_id)
    .bind(episode_number)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| {
        let at = chrono::DateTime::parse_from_rfc3339(&r.fetched_at)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_default();
        (r.segments, at)
    }))
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
          recap_start_ms, recap_end_ms, intro_mixed, ending_mixed, fetched_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(watch_id, episode_number) DO UPDATE SET \
           intro_start_ms = excluded.intro_start_ms, \
           intro_end_ms = excluded.intro_end_ms, \
           ending_start_ms = excluded.ending_start_ms, \
           ending_end_ms = excluded.ending_end_ms, \
           recap_start_ms = excluded.recap_start_ms, \
           recap_end_ms = excluded.recap_end_ms, \
           intro_mixed = excluded.intro_mixed, \
           ending_mixed = excluded.ending_mixed, \
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
    .bind(segments.intro_mixed)
    .bind(segments.ending_mixed)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

/// Trechos detectados localmente (ver `intro_detect`), se o episódio já foi
/// analisado.
pub async fn get_detected(
    pool: &SqlitePool,
    watch_id: i64,
    episode_number: i64,
) -> Result<Option<SkipSegments>, sqlx::Error> {
    sqlx::query_as::<_, SkipSegments>(
        "SELECT intro_start_ms, intro_end_ms, ending_start_ms, ending_end_ms, \
         NULL AS recap_start_ms, NULL AS recap_end_ms \
         FROM detected_segments WHERE watch_id = ? AND episode_number = ?",
    )
    .bind(watch_id)
    .bind(episode_number)
    .fetch_optional(pool)
    .await
}

/// (watch, episódio) → arquivo que foi analisado.
pub async fn detected_paths(
    pool: &SqlitePool,
) -> Result<std::collections::HashMap<(i64, i64), String>, sqlx::Error> {
    let rows: Vec<(i64, i64, String)> =
        sqlx::query_as("SELECT watch_id, episode_number, file_path FROM detected_segments")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(w, e, p)| ((w, e), p)).collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_detected(
    pool: &SqlitePool,
    watch_id: i64,
    episode_number: i64,
    intro: Option<(i64, i64)>,
    ending: Option<(i64, i64)>,
    source: &str,
    file_path: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO detected_segments \
         (watch_id, episode_number, intro_start_ms, intro_end_ms, ending_start_ms, ending_end_ms, \
          source, file_path, analyzed_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(watch_id)
    .bind(episode_number)
    .bind(intro.map(|r| r.0))
    .bind(intro.map(|r| r.1))
    .bind(ending.map(|r| r.0))
    .bind(ending.map(|r| r.1))
    .bind(source)
    .bind(file_path)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}
