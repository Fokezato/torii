use crate::sources::nyaa::NyaaCandidate;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Serialize, FromRow)]
pub struct EpisodeSource {
    pub id: i64,
    pub episode_id: i64,
    pub source_item_id: String,
    pub title: String,
    pub magnet_uri: String,
    pub seeders: Option<i64>,
    pub leechers: Option<i64>,
    pub size: Option<String>,
    pub is_active: i64,
}

/// Grava o release escolhido (`primary`, já marcado ativo) e os outros que
/// também casaram com o mesmo episódio (`alternates`), pra "trocar fonte"
/// não precisar buscar no Nyaa de novo. `INSERT OR IGNORE` porque re-poll
/// pode ver o mesmo candidato de novo (protegido pelo índice único).
pub async fn add_many(
    pool: &SqlitePool,
    episode_id: i64,
    primary: &NyaaCandidate,
    alternates: &[NyaaCandidate],
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    insert_one(pool, episode_id, primary, true, &now).await?;
    for alt in alternates {
        insert_one(pool, episode_id, alt, false, &now).await?;
    }
    Ok(())
}

/// Candidatos achados depois que o episódio já começou a baixar: entram
/// como alternativas (não ativas) pra "trocar fonte" e pro detector de
/// download travado poderem usar.
pub async fn add_alternates(
    pool: &SqlitePool,
    episode_id: i64,
    candidates: &[&NyaaCandidate],
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    for candidate in candidates {
        insert_one(pool, episode_id, candidate, false, &now).await?;
    }
    Ok(())
}

async fn insert_one(
    pool: &SqlitePool,
    episode_id: i64,
    candidate: &NyaaCandidate,
    is_active: bool,
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO episode_sources \
         (episode_id, source_item_id, title, magnet_uri, seeders, leechers, size, is_active, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(episode_id)
    .bind(&candidate.id)
    .bind(&candidate.title)
    .bind(&candidate.magnet)
    .bind(candidate.seeders)
    .bind(candidate.leechers)
    .bind(&candidate.size)
    .bind(is_active)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(pool: &SqlitePool, episode_id: i64) -> Result<Vec<EpisodeSource>, sqlx::Error> {
    sqlx::query_as::<_, EpisodeSource>(
        "SELECT * FROM episode_sources WHERE episode_id = ? ORDER BY seeders DESC",
    )
    .bind(episode_id)
    .fetch_all(pool)
    .await
}

pub async fn get(
    pool: &SqlitePool,
    episode_id: i64,
    source_item_id: &str,
) -> Result<EpisodeSource, sqlx::Error> {
    sqlx::query_as::<_, EpisodeSource>(
        "SELECT * FROM episode_sources WHERE episode_id = ? AND source_item_id = ?",
    )
    .bind(episode_id)
    .bind(source_item_id)
    .fetch_one(pool)
    .await
}

pub async fn set_active(pool: &SqlitePool, episode_id: i64, source_item_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episode_sources SET is_active = 0 WHERE episode_id = ?")
        .bind(episode_id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE episode_sources SET is_active = 1 WHERE episode_id = ? AND source_item_id = ?")
        .bind(episode_id)
        .bind(source_item_id)
        .execute(pool)
        .await?;
    Ok(())
}
