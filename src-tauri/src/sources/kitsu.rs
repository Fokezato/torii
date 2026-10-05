use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const API: &str = "https://kitsu.io/api/edge";
const PAGE: usize = 20;
const MAX_PAGES: usize = 15;
const CACHE_TTL: Duration = Duration::from_secs(12 * 3600);

#[derive(Debug, Clone, Serialize)]
pub struct EpisodeMeta {
    pub number: i64,
    pub title: Option<String>,
    pub synopsis: Option<String>,
    pub thumbnail: Option<String>,
}

static CACHE: LazyLock<Mutex<HashMap<i64, (Instant, Vec<EpisodeMeta>)>>> = LazyLock::new(Default::default);

#[derive(Deserialize)]
struct Doc<T> {
    data: T,
}

#[derive(Deserialize)]
struct Mapping {
    relationships: MappingRel,
}

#[derive(Deserialize)]
struct MappingRel {
    item: Doc<Ref>,
}

#[derive(Deserialize)]
struct Ref {
    id: String,
}

#[derive(Deserialize)]
struct Episode {
    attributes: Attrs,
}

#[derive(Deserialize)]
struct Attrs {
    number: Option<i64>,
    #[serde(rename = "canonicalTitle")]
    canonical_title: Option<String>,
    #[serde(default)]
    titles: HashMap<String, Option<String>>,
    synopsis: Option<String>,
    thumbnail: Option<Thumb>,
}

#[derive(Deserialize)]
struct Thumb {
    medium: Option<String>,
    original: Option<String>,
}

async fn get<T: for<'de> Deserialize<'de>>(client: &reqwest::Client, url: &str) -> Result<T, String> {
    let resp = client
        .get(url)
        .header("Accept", "application/vnd.api+json")
        .header("User-Agent", "Torii")
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Kitsu HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Per-episode title, synopsis and thumbnail. Empty when Kitsu has no data for the show.
pub async fn episodes(client: &reqwest::Client, anilist_id: i64) -> Result<Vec<EpisodeMeta>, String> {
    if let Some((at, hit)) = CACHE.lock().unwrap().get(&anilist_id) {
        if at.elapsed() < CACHE_TTL {
            return Ok(hit.clone());
        }
    }

    let mappings: Doc<Vec<Mapping>> = get(
        client,
        &format!("{API}/mappings?filter[externalSite]=anilist/anime&filter[externalId]={anilist_id}&include=item"),
    )
    .await?;
    let Some(kitsu_id) = mappings.data.into_iter().next().map(|m| m.relationships.item.data.id) else {
        CACHE.lock().unwrap().insert(anilist_id, (Instant::now(), Vec::new()));
        return Ok(Vec::new());
    };

    let mut out = Vec::new();
    for page in 0..MAX_PAGES {
        let batch: Doc<Vec<Episode>> = get(
            client,
            &format!("{API}/anime/{kitsu_id}/episodes?page[limit]={PAGE}&page[offset]={}&sort=number", page * PAGE),
        )
        .await?;
        let len = batch.data.len();
        for ep in batch.data {
            let a = ep.attributes;
            let Some(number) = a.number else { continue };
            let title = non_empty(a.canonical_title)
                .or_else(|| a.titles.get("en_us").cloned().flatten())
                .or_else(|| a.titles.get("en").cloned().flatten());
            let thumbnail = a.thumbnail.and_then(|t| t.medium.or(t.original));
            out.push(EpisodeMeta { number, title: non_empty(title), synopsis: non_empty(a.synopsis.map(|s| super::anilist::strip_credits(&s))), thumbnail });
        }
        if len < PAGE {
            break;
        }
    }
    CACHE.lock().unwrap().insert(anilist_id, (Instant::now(), out.clone()));
    Ok(out)
}
