use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct SystemInfo {
    #[serde(rename = "ServerName")]
    server_name: Option<String>,
    #[serde(rename = "Version")]
    version: Option<String>,
}

pub struct ConnectionInfo {
    pub server_name: String,
    pub version: String,
}

/// Testa a conexão batendo em `/System/Info` — endpoint mais barato que
/// exige um token válido, então também confirma a API key, não só que o
/// servidor tá de pé.
pub async fn test_connection(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<ConnectionInfo, String> {
    let url = format!("{}/System/Info", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .header("X-Emby-Token", api_key)
        .send()
        .await
        .map_err(|e| tr!("não deu pra conectar: {e}", "couldn't connect: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Jellyfin respondeu {}", resp.status()));
    }

    let info: SystemInfo = resp.json().await.map_err(|e| format!("resposta inesperada: {e}"))?;
    Ok(ConnectionInfo {
        server_name: info.server_name.unwrap_or_else(|| "Jellyfin".to_string()),
        version: info.version.unwrap_or_default(),
    })
}

/// Pede pro Jellyfin re-escanear uma pasta específica (não a biblioteca
/// inteira) depois que um episódio termina de baixar.
pub async fn refresh_path(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    path: &str,
) -> Result<(), String> {
    let url = format!("{}/Library/Media/Updated", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "Updates": [{ "Path": path, "UpdateType": "Created" }]
    });
    let resp = client
        .post(&url)
        .header("X-Emby-Token", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Jellyfin respondeu {} ao pedir refresh", resp.status()));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ItemsResponse {
    #[serde(rename = "Items")]
    items: Vec<JellyfinItem>,
}

#[derive(Debug, Deserialize)]
struct JellyfinItem {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Path")]
    path: Option<String>,
}

/// Espera o Jellyfin terminar de escanear e acha o item que corresponde ao
/// arquivo que acabou de baixar, comparando o path (case-insensitive, já
/// que Windows não diferencia maiúscula/minúscula). Poll curto porque o
/// refresh do Jellyfin não é instantâneo.
pub async fn find_item_by_path(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    file_path: &str,
) -> Result<Option<String>, String> {
    let url = format!(
        "{}/Items?Recursive=true&IncludeItemTypes=Episode&Fields=Path",
        base_url.trim_end_matches('/')
    );
    let target = file_path.to_lowercase();

    for attempt in 0..6 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        let resp = client
            .get(&url)
            .header("X-Emby-Token", api_key)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            continue;
        }
        let parsed: ItemsResponse = match resp.json().await {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Some(item) = parsed
            .items
            .into_iter()
            .find(|i| i.path.as_deref().map(|p| p.to_lowercase()) == Some(target.clone()))
        {
            return Ok(Some(item.id));
        }
    }
    Ok(None)
}

pub async fn delete_item(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    item_id: &str,
) -> Result<(), String> {
    let url = format!("{}/Items/{item_id}", base_url.trim_end_matches('/'));
    let resp = client
        .delete(&url)
        .header("X-Emby-Token", api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Jellyfin respondeu {} ao apagar item", resp.status()));
    }
    Ok(())
}
