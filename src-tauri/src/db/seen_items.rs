use sqlx::SqlitePool;
use std::collections::HashSet;

pub async fn get_seen_ids(pool: &SqlitePool, watch_id: i64) -> Result<HashSet<String>, sqlx::Error> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT source_item_id FROM seen_items WHERE watch_id = ?")
            .bind(watch_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

pub async fn mark_seen(
    pool: &SqlitePool,
    watch_id: i64,
    source_item_id: &str,
    title: &str,
    matched: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO seen_items (source_item_id, watch_id, title, matched, seen_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(source_item_id)
    .bind(watch_id)
    .bind(title)
    .bind(matched)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}
