use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};

const API_URL: &str = "https://graphql.anilist.co";

const MEDIA_FIELDS: &str = "id title { romaji english native } coverImage { extraLarge } bannerImage \
episodes status genres averageScore duration description(asHtml: false) season seasonYear \
studios(isMain: true) { nodes { name } } nextAiringEpisode { airingAt episode } \
airingSchedule(notYetAired: true, perPage: 50) { nodes { airingAt episode } }";

const SEASON_QUERY_PREFIX: &str = "query ($season: MediaSeason, $seasonYear: Int, $page: Int) { \
Page(page: $page, perPage: 50) { \
media(season: $season, seasonYear: $seasonYear, type: ANIME, sort: POPULARITY_DESC, format_in: [TV, TV_SHORT]) { ";

const TRENDING_QUERY_PREFIX: &str = "query ($page: Int) { \
Page(page: $page, perPage: 20) { \
media(sort: TRENDING_DESC, type: ANIME, format_in: [TV, TV_SHORT]) { ";

const SEARCH_QUERY_PREFIX: &str = "query ($search: String) { \
Page(page: 1, perPage: 20) { \
media(search: $search, type: ANIME, sort: POPULARITY_DESC) { ";

const BY_IDS_QUERY_PREFIX: &str = "query ($ids: [Int]) { \
Page(page: 1, perPage: 50) { \
media(id_in: $ids, type: ANIME) { ";

const QUERY_SUFFIX: &str = " } } }";

fn build_query(prefix: &str) -> String {
    format!("{prefix}{MEDIA_FIELDS}{QUERY_SUFFIX}")
}

#[derive(Debug, Clone, Serialize)]
pub struct AnimeSummary {
    pub anilist_id: i32,
    pub title: String,
    pub cover_url: Option<String>,
    pub banner_url: Option<String>,
    pub episodes: Option<i32>,
    pub status: Option<String>,
    pub genres: Vec<String>,
    pub score: Option<i32>,
    pub duration: Option<i32>,
    pub studio: Option<String>,
    pub description: Option<String>,
    pub season: Option<String>,
    pub season_year: Option<i32>,
    pub next_airing_at: Option<i64>,
    pub next_airing_episode: Option<i32>,
    /// Episódio que a AniList já sabe a data de exibição mas ainda não foi
    /// ao ar — só os "próximos" (`airingSchedule(notYetAired: true)`), não
    /// o histórico inteiro. Usado na Biblioteca pra mostrar "Disponível a
    /// partir de DD/MM" no lugar de "Procurando" em placeholder de episódio
    /// que fisicamente ainda não existe pra baixar (ver Torii issue: Slime
    /// S4 com episódio futuro aparecendo como "procurando" sem sentido).
    pub upcoming_episodes: Vec<UpcomingEpisode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpcomingEpisode {
    pub episode: i32,
    pub airing_at: i64,
}

#[derive(Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Deserialize)]
struct PageData {
    #[serde(rename = "Page")]
    page: PageMedia,
}

#[derive(Deserialize)]
struct PageMedia {
    media: Vec<RawMedia>,
}

#[derive(Deserialize)]
struct RawMedia {
    id: i32,
    title: RawTitle,
    #[serde(rename = "coverImage")]
    cover_image: Option<RawCover>,
    #[serde(rename = "bannerImage")]
    banner_image: Option<String>,
    episodes: Option<i32>,
    status: Option<String>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(rename = "averageScore")]
    average_score: Option<i32>,
    duration: Option<i32>,
    description: Option<String>,
    studios: Option<RawStudioConnection>,
    season: Option<String>,
    #[serde(rename = "seasonYear")]
    season_year: Option<i32>,
    #[serde(rename = "nextAiringEpisode")]
    next_airing_episode: Option<RawAiring>,
    #[serde(rename = "airingSchedule")]
    airing_schedule: Option<RawAiringScheduleConnection>,
}

#[derive(Deserialize)]
struct RawAiringScheduleConnection {
    nodes: Vec<RawAiring>,
}

#[derive(Deserialize)]
struct RawAiring {
    #[serde(rename = "airingAt")]
    airing_at: i64,
    episode: i32,
}

#[derive(Deserialize)]
struct RawTitle {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}

#[derive(Deserialize)]
struct RawCover {
    #[serde(rename = "extraLarge")]
    extra_large: Option<String>,
}

#[derive(Deserialize)]
struct RawStudioConnection {
    nodes: Vec<RawStudio>,
}

#[derive(Deserialize)]
struct RawStudio {
    name: String,
}

/// Remove tags HTML simples que a AniList às vezes deixa na descrição mesmo com asHtml:false.
fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

impl From<RawMedia> for AnimeSummary {
    fn from(m: RawMedia) -> Self {
        let title = m
            .title
            .english
            .or(m.title.romaji)
            .or(m.title.native)
            .unwrap_or_else(|| "Sem título".to_string());
        AnimeSummary {
            anilist_id: m.id,
            title,
            cover_url: m.cover_image.and_then(|c| c.extra_large),
            banner_url: m.banner_image,
            episodes: m.episodes,
            status: m.status,
            genres: m.genres,
            score: m.average_score,
            duration: m.duration,
            studio: m.studios.and_then(|s| s.nodes.into_iter().next()).map(|s| s.name),
            description: m.description.map(|d| strip_html(&d)).filter(|d| !d.is_empty()),
            season: m.season,
            season_year: m.season_year,
            next_airing_at: m.next_airing_episode.as_ref().map(|a| a.airing_at),
            next_airing_episode: m.next_airing_episode.map(|a| a.episode),
            upcoming_episodes: m
                .airing_schedule
                .map(|c| {
                    c.nodes
                        .into_iter()
                        .map(|a| UpcomingEpisode { episode: a.episode, airing_at: a.airing_at })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

/// (season, year) no formato esperado pela AniList. Dezembro rola pro WINTER do ano seguinte.
pub fn current_season_year() -> (&'static str, i32) {
    let now = Local::now();
    let month = now.month();
    let year = now.year();
    match month {
        12 => ("WINTER", year + 1),
        1 | 2 => ("WINTER", year),
        3..=5 => ("SPRING", year),
        6..=8 => ("SUMMER", year),
        _ => ("FALL", year),
    }
}

async fn graphql_query(
    client: &reqwest::Client,
    query: &str,
    variables: serde_json::Value,
) -> Result<Vec<AnimeSummary>, String> {
    let body = serde_json::json!({ "query": query, "variables": variables });
    let resp = client
        .post(API_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let parsed: GraphQlResponse<PageData> = resp.json().await.map_err(|e| e.to_string())?;
    if let Some(errors) = parsed.errors {
        let msg = errors
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(msg);
    }
    let data = parsed.data.ok_or_else(|| "resposta vazia da AniList".to_string())?;
    Ok(data.page.media.into_iter().map(AnimeSummary::from).collect())
}

pub async fn browse_season(
    client: &reqwest::Client,
    season: Option<&str>,
    year: Option<i32>,
) -> Result<Vec<AnimeSummary>, String> {
    let (default_season, default_year) = current_season_year();
    let season = season.unwrap_or(default_season);
    let year = year.unwrap_or(default_year);
    graphql_query(
        client,
        &build_query(SEASON_QUERY_PREFIX),
        serde_json::json!({ "season": season, "seasonYear": year, "page": 1 }),
    )
    .await
}

pub async fn browse_trending(client: &reqwest::Client) -> Result<Vec<AnimeSummary>, String> {
    graphql_query(client, &build_query(TRENDING_QUERY_PREFIX), serde_json::json!({ "page": 1 })).await
}

pub async fn search(client: &reqwest::Client, q: &str) -> Result<Vec<AnimeSummary>, String> {
    graphql_query(client, &build_query(SEARCH_QUERY_PREFIX), serde_json::json!({ "search": q })).await
}

pub async fn by_ids(client: &reqwest::Client, ids: &[i32]) -> Result<Vec<AnimeSummary>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    graphql_query(client, &build_query(BY_IDS_QUERY_PREFIX), serde_json::json!({ "ids": ids })).await
}

const RELATIONS_QUERY: &str = "query ($id: Int) { Media(id: $id, type: ANIME) { \
relations { edges { relationType(version: 2) node { id type format } } } } }";

/// Formatos que contam como "temporada" (filme/OVA/especial ficam de fora).
const SEASON_FORMATS: [&str; 3] = ["TV", "TV_SHORT", "ONA"];
/// Trava de segurança contra franquia gigante (ex. Gintama, Detective Conan).
const MAX_FRANCHISE_SEASONS: usize = 20;

#[derive(Deserialize)]
struct RelationsData {
    #[serde(rename = "Media")]
    media: Option<RelationsMedia>,
}

#[derive(Deserialize)]
struct RelationsMedia {
    relations: RelationConnection,
}

#[derive(Deserialize)]
struct RelationConnection {
    edges: Vec<RelationEdge>,
}

#[derive(Deserialize)]
struct RelationEdge {
    #[serde(rename = "relationType")]
    relation_type: Option<String>,
    node: RelationNode,
}

#[derive(Deserialize)]
struct RelationNode {
    id: i32,
    #[serde(rename = "type")]
    media_type: Option<String>,
    format: Option<String>,
}

async fn season_neighbors(client: &reqwest::Client, id: i32) -> Result<Vec<i32>, String> {
    let body = serde_json::json!({ "query": RELATIONS_QUERY, "variables": { "id": id } });
    let resp = client.post(API_URL).json(&body).send().await.map_err(|e| e.to_string())?;
    let parsed: GraphQlResponse<RelationsData> = resp.json().await.map_err(|e| e.to_string())?;
    if let Some(errors) = parsed.errors {
        return Err(errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; "));
    }
    let Some(media) = parsed.data.and_then(|d| d.media) else {
        return Ok(Vec::new());
    };
    Ok(media
        .relations
        .edges
        .into_iter()
        .filter(|e| matches!(e.relation_type.as_deref(), Some("SEQUEL") | Some("PREQUEL")))
        .filter(|e| e.node.media_type.as_deref() == Some("ANIME"))
        .filter(|e| e.node.format.as_deref().is_some_and(|f| SEASON_FORMATS.contains(&f)))
        .map(|e| e.node.id)
        .collect())
}

fn season_order(season: Option<&str>) -> u8 {
    match season {
        Some("WINTER") => 0,
        Some("SPRING") => 1,
        Some("SUMMER") => 2,
        Some("FALL") => 3,
        _ => 4,
    }
}

/// Todas as temporadas (TV) do mesmo anime, em ordem de lançamento — segue
/// as ligações de sequência/prequel da AniList a partir de qualquer
/// temporada. A 1ª da lista é a "raiz" (vira o anime na Biblioteca). Não
/// depende do título: "Demon Slayer ... Entertainment District Arc" não tem
/// "Season 2" no nome e mesmo assim é achada como temporada 2.
pub async fn franchise_seasons(client: &reqwest::Client, anilist_id: i32) -> Result<Vec<AnimeSummary>, String> {
    let mut seen = vec![anilist_id];
    let mut queue = std::collections::VecDeque::from([anilist_id]);
    while let Some(id) = queue.pop_front() {
        if seen.len() >= MAX_FRANCHISE_SEASONS {
            break;
        }
        for neighbor in season_neighbors(client, id).await? {
            if !seen.contains(&neighbor) && seen.len() < MAX_FRANCHISE_SEASONS {
                seen.push(neighbor);
                queue.push_back(neighbor);
            }
        }
    }
    let mut seasons = by_ids(client, &seen).await?;
    // Sem ano (anunciado, sem data) vai pro fim.
    seasons.sort_by_key(|s| (s.season_year.unwrap_or(i32::MAX), season_order(s.season.as_deref())));
    Ok(seasons)
}
