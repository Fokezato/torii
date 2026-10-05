use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};

const API_URL: &str = "https://graphql.anilist.co";

const MEDIA_FIELDS: &str = "id title { romaji english native } coverImage { extraLarge } bannerImage \
episodes status format startDate { year month } genres averageScore duration description(asHtml: false) season seasonYear \
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
    pub format: Option<String>,
    pub start_year: Option<i32>,
    pub start_month: Option<i32>,
    pub next_airing_at: Option<i64>,
    pub next_airing_episode: Option<i32>,
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
    #[serde(rename = "pageInfo")]
    page_info: Option<RawPageInfo>,
}

#[derive(Deserialize)]
struct RawPageInfo {
    #[serde(rename = "hasNextPage")]
    has_next_page: bool,
    total: Option<i64>,
}

#[derive(Deserialize)]
struct RawDate {
    year: Option<i32>,
    month: Option<i32>,
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
    format: Option<String>,
    #[serde(rename = "startDate")]
    start_date: Option<RawDate>,
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

/// Drops credits like "(Source: Crunchyroll)" or "[Written by MAL Rewrite]".
pub fn strip_credits(text: &str) -> String {
    static CREDIT: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)\s*[(\[]\s*(?:source|written by)\b[^)\]]*[)\]]").unwrap()
    });
    CREDIT.replace_all(text, "").trim().to_string()
}

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
            .unwrap_or_else(|| tr!("Sem título", "Untitled"));
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
            description: m.description.map(|d| strip_credits(&strip_html(&d))).filter(|d| !d.is_empty()),
            season: m.season,
            season_year: m.season_year,
            format: m.format,
            start_year: m.start_date.as_ref().and_then(|d| d.year),
            start_month: m.start_date.as_ref().and_then(|d| d.month),
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
    graphql_page(client, query, variables).await.map(|(items, _, _)| items)
}

async fn graphql_page(
    client: &reqwest::Client,
    query: &str,
    variables: serde_json::Value,
) -> Result<(Vec<AnimeSummary>, bool, Option<i64>), String> {
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
    let data = parsed.data.ok_or_else(|| tr!("resposta vazia da AniList", "empty response from AniList"))?;
    let has_next = data.page.page_info.as_ref().is_some_and(|p| p.has_next_page);
    let total = data.page.page_info.and_then(|p| p.total);
    Ok((data.page.media.into_iter().map(AnimeSummary::from).collect(), has_next, total))
}

const CATALOG_QUERY_PREFIX: &str = "query ($page: Int, $search: String, $genres: [String], $year: Int, $season: MediaSeason, $formats: [MediaFormat], $status: MediaStatus, $minScore: Int, $sort: [MediaSort]) { Page(page: $page, perPage: 30) { pageInfo { hasNextPage total } media(type: ANIME, isAdult: false, search: $search, genre_in: $genres, seasonYear: $year, season: $season, format_in: $formats, status: $status, averageScore_greater: $minScore, sort: $sort) { ";

#[derive(Debug, Default, Deserialize)]
pub struct CatalogFilter {
    pub search: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    pub year: Option<i32>,
    pub season: Option<String>,
    #[serde(default)]
    pub formats: Vec<String>,
    pub status: Option<String>,
    pub min_score: Option<i32>,
    pub sort: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CatalogPage {
    pub items: Vec<AnimeSummary>,
    pub has_next: bool,
    pub total: Option<i64>,
}

pub async fn catalog(client: &reqwest::Client, filter: &CatalogFilter, page: i32) -> Result<CatalogPage, String> {
    let search = filter.search.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let mut sort = vec![filter.sort.clone().unwrap_or_else(|| "POPULARITY_DESC".to_string())];
    if search.is_some() {
        sort.insert(0, "SEARCH_MATCH".to_string());
    }
    // AniList answers 500 to explicit nulls: only send the filters that are set.
    let mut variables = serde_json::json!({ "page": page, "sort": sort });
    let vars = variables.as_object_mut().unwrap();
    if let Some(search) = search {
        vars.insert("search".into(), search.into());
    }
    if !filter.genres.is_empty() {
        vars.insert("genres".into(), filter.genres.clone().into());
    }
    if let Some(year) = filter.year {
        vars.insert("year".into(), year.into());
    }
    if let Some(season) = &filter.season {
        vars.insert("season".into(), season.clone().into());
    }
    if !filter.formats.is_empty() {
        vars.insert("formats".into(), filter.formats.clone().into());
    }
    if let Some(status) = &filter.status {
        vars.insert("status".into(), status.clone().into());
    }
    if let Some(min) = filter.min_score {
        vars.insert("minScore".into(), min.into());
    }
    let (items, has_next, total) = graphql_page(client, &build_query(CATALOG_QUERY_PREFIX), variables).await?;
    Ok(CatalogPage { items, has_next, total })
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

const SEASON_FORMATS: [&str; 3] = ["TV", "TV_SHORT", "ONA"];
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
    seasons.sort_by_key(|s| (s.season_year.unwrap_or(i32::MAX), season_order(s.season.as_deref())));
    Ok(seasons)
}

#[cfg(test)]
mod tests {
    use super::strip_credits;

    #[test]
    fn credits_are_removed_from_synopses() {
        assert_eq!(strip_credits("The battle ignites. (Source: Crunchyroll)"), "The battle ignites.");
        assert_eq!(strip_credits("A story.\n\n[Written by MAL Rewrite]"), "A story.");
        assert_eq!(strip_credits("He returns (again) home."), "He returns (again) home.");
    }
}
