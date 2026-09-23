use serde::Serialize;

const FEED_URL: &str = "https://www.animenewsnetwork.com/news/rss.xml";

#[derive(Debug, Clone, Serialize)]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub summary: Option<String>,
    pub category: Option<String>,
    pub published_at: Option<String>,
}

pub async fn fetch(client: &reqwest::Client) -> Result<Vec<NewsItem>, String> {
    let bytes = client
        .get(FEED_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;

    let feed = feed_rs::parser::parse(bytes.as_ref()).map_err(|e| e.to_string())?;

    let items = feed
        .entries
        .into_iter()
        .take(12)
        .map(|entry| NewsItem {
            title: entry.title.map(|t| t.content).unwrap_or_else(|| "Sem título".to_string()),
            link: entry.links.into_iter().next().map(|l| l.href).unwrap_or_default(),
            summary: entry.summary.map(|s| s.content),
            category: entry.categories.into_iter().next().map(|c| c.term),
            published_at: entry.published.map(|d| d.to_rfc3339()),
        })
        .collect();

    Ok(items)
}
