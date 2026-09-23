//! Tradução automática das sinopses (a AniList só tem inglês). Usa o
//! endpoint público do Google Tradutor e guarda o resultado no banco, então
//! cada sinopse é traduzida uma vez só.

use sqlx::SqlitePool;

const ENDPOINT: &str = "https://translate.googleapis.com/translate_a/single?client=gtx&sl=en&dt=t";

pub async fn cached(pool: &SqlitePool, source: &str, target: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT translated FROM translations WHERE source = ? AND target = ?")
        .bind(source)
        .bind(target)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

async fn store(pool: &SqlitePool, source: &str, target: &str, translated: &str) {
    let _ = sqlx::query(
        "INSERT OR REPLACE INTO translations (source, target, translated, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(source)
    .bind(target)
    .bind(translated)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await;
}

async fn fetch(http: &reqwest::Client, text: &str, target: &str) -> Result<String, String> {
    let resp = http
        .post(format!("{ENDPOINT}&tl={target}"))
        .form(&[("q", text)])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    parse_response(&body).ok_or_else(|| "unexpected translation response".to_string())
}

/// Resposta: `[[["trecho traduzido", "trecho original", ...], ...], ...]`.
fn parse_response(body: &serde_json::Value) -> Option<String> {
    let text: String = body
        .get(0)?
        .as_array()?
        .iter()
        .filter_map(|chunk| chunk.get(0)?.as_str())
        .collect();
    (!text.trim().is_empty()).then_some(text)
}

/// Texto traduzido pra `target` ("pt"); se falhar, devolve o original.
pub async fn translate(http: &reqwest::Client, pool: &SqlitePool, text: &str, target: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    if let Some(hit) = cached(pool, text, target).await {
        return hit;
    }
    match fetch(http, text, target).await {
        Ok(translated) => {
            store(pool, text, target, &translated).await;
            translated
        }
        Err(_) => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_response;

    #[test]
    fn joins_translated_chunks() {
        let body = serde_json::json!([[["Olá. ", "Hello. ", null], ["Mundo", "World", null]], null, "en"]);
        assert_eq!(parse_response(&body).as_deref(), Some("Olá. Mundo"));
    }

    #[test]
    fn rejects_empty_response() {
        assert_eq!(parse_response(&serde_json::json!([])), None);
    }
}
