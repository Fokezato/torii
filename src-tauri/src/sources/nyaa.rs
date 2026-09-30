use futures::stream::{self, StreamExt};
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

const BASE_URL: &str = "https://nyaa.si";
const CATEGORY: &str = "1_2";
const DETAIL_FETCH_CONCURRENCY: usize = 3;
const EPISODE_PROBE_MAX: u32 = 24;
const EPISODE_PROBE_CONCURRENCY: usize = 5;

#[derive(Debug, Clone, Serialize)]
pub struct NyaaCandidate {
    pub id: String,
    pub title: String,
    pub magnet: String,
    pub view_url: String,
    pub size: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub published_at: Option<String>,
}

pub struct MatchResult {
    pub matched: Vec<EpisodeMatch>,
    pub all_new: Vec<NyaaCandidate>,
    /// Releases without an episode number (season packs) that passed every filter.
    pub batches: Vec<NyaaCandidate>,
    pub rejected: Rejections,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Rejections {
    pub quality: usize,
    pub season: usize,
    pub range: usize,
    pub other_work: usize,
    pub language: usize,
}

pub struct EpisodeMatch {
    pub primary: NyaaCandidate,
    pub alternates: Vec<NyaaCandidate>,
}

#[derive(Debug, Deserialize)]
struct RssRoot {
    channel: RssChannel,
}

#[derive(Debug, Deserialize)]
struct RssChannel {
    #[serde(rename = "item", default)]
    items: Vec<RssItem>,
}

#[derive(Debug, Deserialize)]
struct RssItem {
    title: String,
    guid: RssGuid,
    #[serde(rename = "pubDate")]
    pub_date: Option<String>,
    seeders: Option<i32>,
    leechers: Option<i32>,
    #[serde(rename = "infoHash")]
    info_hash: Option<String>,
    size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RssGuid {
    #[serde(rename = "$text")]
    text: String,
}

fn fallback_magnet(info_hash: &str, title: &str) -> String {
    format!(
        "magnet:?xt=urn:btih:{}&dn={}&tr={}&tr={}",
        info_hash,
        urlencoding::encode(title),
        urlencoding::encode("udp://open.stealth.si:80/announce"),
        urlencoding::encode("udp://tracker.opentrackr.org:1337/announce"),
    )
}

fn parse_rss(text: &str) -> Result<Vec<NyaaCandidate>, String> {
    let root: RssRoot = quick_xml::de::from_str(text).map_err(|e| e.to_string())?;

    Ok(root
        .channel
        .items
        .into_iter()
        .map(|item| {
            let id = item
                .guid
                .text
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            let magnet = item
                .info_hash
                .as_deref()
                .map(|h| fallback_magnet(h, &item.title))
                .unwrap_or_default();
            NyaaCandidate {
                id,
                title: item.title,
                magnet,
                view_url: item.guid.text,
                size: item.size,
                seeders: item.seeders,
                leechers: item.leechers,
                published_at: item.pub_date,
            }
        })
        .collect())
}

fn sanitize_query(query: &str) -> String {
    query.replace('-', " ")
}

pub async fn search(client: &reqwest::Client, query: &str) -> Result<Vec<NyaaCandidate>, String> {
    search_in(client, query, CATEGORY).await
}

async fn search_in(client: &reqwest::Client, query: &str, category: &str) -> Result<Vec<NyaaCandidate>, String> {
    let query = sanitize_query(query);
    let url = format!(
        "{BASE_URL}/?page=rss&c={category}&q={}",
        urlencoding::encode(&query)
    );
    let text = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    parse_rss(&text)
}

fn episode_probe_queries(base_query: &str, season: u32, start: Option<i64>, end: Option<i64>) -> Vec<String> {
    let lo = start.unwrap_or(1).max(1) as u32;
    let hi = end.unwrap_or(EPISODE_PROBE_MAX as i64).clamp(1, EPISODE_PROBE_MAX as i64) as u32;
    if lo > hi {
        return Vec::new();
    }
    (lo..=hi).map(|ep| format!("{base_query} S{season:02}E{ep:02}")).collect()
}

async fn search_with_episode_probes(
    client: &reqwest::Client,
    base_query: &str,
    season: u32,
    episode_start: Option<i64>,
    episode_end: Option<i64>,
    base: Vec<NyaaCandidate>,
) -> Vec<NyaaCandidate> {
    let queries = episode_probe_queries(base_query, season, episode_start, episode_end);
    let probed: Vec<NyaaCandidate> = stream::iter(queries)
        .map(|q| {
            let client = client.clone();
            async move { search(&client, &q).await.unwrap_or_default() }
        })
        .buffer_unordered(EPISODE_PROBE_CONCURRENCY)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect();

    let mut seen_ids: HashSet<String> = base.iter().map(|c| c.id.clone()).collect();
    let mut merged = base;
    for candidate in probed {
        if seen_ids.insert(candidate.id.clone()) {
            merged.push(candidate);
        }
    }
    merged
}

pub fn split_season(query: &str) -> (String, Option<u32>) {
    let re =
        Regex::new(r"(?i)\s*(?:season\s*(\d+)|(\d+)(?:st|nd|rd|th)\s*season)\s*$").unwrap();
    match re.captures(query) {
        Some(caps) => {
            let season = caps
                .get(1)
                .or_else(|| caps.get(2))
                .and_then(|m| m.as_str().parse::<u32>().ok());
            let stripped = re.replace(query, "").trim().to_string();
            (stripped, season)
        }
        None => (query.to_string(), None),
    }
}

fn title_matches_season(title: &str, season: u32) -> bool {
    let pattern = format!(r"(?i)\bs0*{season}(?:e\d+)?\b|\bseason\s*0*{season}\b");
    Regex::new(&pattern).map(|r| r.is_match(title)).unwrap_or(true)
}

static SXXEYY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)S(\d{1,2})E(\d{1,3})").unwrap());
// "Show - 05 [1080p]", "Show - 12v2 (WEB)", "Show - 05.mkv"
static DASH_EPISODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s-\s(\d{1,4})(?:v\d+)?(?:\s*[\[(]|\s*\.[a-z0-9]{2,4}$|\s*$)").unwrap()
});
static EP_EPISODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bEP?\.?\s?(\d{2,3})\b").unwrap());

pub fn extract_episode_number(title: &str) -> Option<u32> {
    [&*SXXEYY, &*DASH_EPISODE, &*EP_EPISODE].iter().find_map(|re| {
        let caps = re.captures(title)?;
        caps.get(caps.len() - 1)?.as_str().parse().ok()
    })
}

pub fn extract_season_number(title: &str) -> Option<u32> {
    SXXEYY.captures(title)?.get(1)?.as_str().parse().ok()
}

fn normalize_words(text: &str) -> String {
    static LEADING_TAGS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*(?:\[[^\]]*\]\s*|\([^)]*\)\s*)+").unwrap());
    let text = LEADING_TAGS.replace(text, "");
    let mapped: String = text
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A release of another work in the same franchise: "Steins;Gate 0", "... The Movie",
/// "... OVA", or another season. `names` are the anime's own titles; without a match
/// nothing is rejected.
pub fn is_other_work(title: &str, names: &[String], season: u32) -> bool {
    const MARKERS: [&str; 10] = ["movie", "film", "gekijouban", "ova", "ovas", "oad", "special", "specials", "zero", "recap"];
    static SEASON_TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^s(\d{1,2})$").unwrap());
    static ORDINAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,2})(?:st|nd|rd|th)$").unwrap());
    let norm = normalize_words(title);
    for name in names {
        let name = normalize_words(name);
        if name.is_empty() {
            continue;
        }
        let Some(pos) = norm.match_indices(&name).map(|(i, _)| i).find(|&i| {
            let end = i + name.len();
            (i == 0 || norm.as_bytes()[i - 1] == b' ') && (end == norm.len() || norm.as_bytes()[end] == b' ')
        }) else {
            continue;
        };
        let rest: Vec<&str> = norm[pos + name.len()..].split_whitespace().take(2).collect();
        let (t0, t1) = (rest.first().copied().unwrap_or(""), rest.get(1).copied().unwrap_or(""));
        let other_season = |n: &str| n.parse::<u32>().is_ok_and(|n| n != season);
        let before = norm[..pos].split_whitespace().last().unwrap_or("");
        return MARKERS.contains(&t0)
            || (t0 == "the" && MARKERS.contains(&t1))
            || ["gekijouban", "movie", "film"].contains(&before)
            || (t0.len() == 1 && other_season(t0))
            || (t0 == "season" && other_season(t1))
            || SEASON_TOKEN.captures(t0).is_some_and(|c| other_season(&c[1]))
            || (t1 == "season" && ORDINAL.captures(t0).is_some_and(|c| other_season(&c[1])));
    }
    false
}

const VIDEO_EXTENSIONS: [&str; 6] = ["mkv", "mp4", "avi", "webm", "m4v", "ts"];

/// Episode number of a file inside a season pack, or `None` for extras, other
/// seasons/works and non-video files.
pub fn pack_file_episode(path: &str, names: &[String], season: u32) -> Option<u32> {
    static EXTRA_DIR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(?:extras?|specials?|bonus|ova|oad|nc ?op|nc ?ed|creditless|menus?|pv|previews?|trailers?|scans|fonts)\b").unwrap()
    });
    static LEADING_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,3})(?:v\d+)?(?:[\s._-]|$)").unwrap());
    let components: Vec<&str> = path.split(['/', '\\']).collect();
    let file = components.last()?;
    let (stem, ext) = file.rsplit_once('.')?;
    if !VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
        return None;
    }
    // The pack's root folder often reads like "Show (S1 + OVA + Movie)": folders named
    // after the anime are not treated as extras.
    let named_after_anime = |dir: &str| {
        let dir = normalize_words(dir);
        names.iter().map(|n| normalize_words(n)).any(|n| !n.is_empty() && dir.contains(&n))
    };
    let extra_dir = components[..components.len() - 1]
        .iter()
        .any(|dir| !named_after_anime(dir) && EXTRA_DIR.is_match(dir));
    if extra_dir || EXTRA_DIR.is_match(stem) {
        return None;
    }
    if extract_season_number(stem).is_some_and(|s| s != season) || is_other_work(stem, names, season) {
        return None;
    }
    extract_episode_number(file)
        .or_else(|| LEADING_NUMBER.captures(stem).and_then(|c| c[1].parse().ok()))
}

fn title_matches_episode_range(title: &str, start: Option<i64>, end: Option<i64>) -> bool {
    if start.is_none() && end.is_none() {
        return true;
    }
    let Some(ep) = extract_episode_number(title) else {
        return true;
    };
    let ep = ep as i64;
    start.map_or(true, |s| ep >= s) && end.map_or(true, |e| ep <= e)
}

pub fn matches_quality(title: &str, quality: &str) -> bool {
    if quality.is_empty() || quality.eq_ignore_ascii_case("any") {
        return true;
    }
    let pattern = if quality.eq_ignore_ascii_case("2160p") {
        "(?i)2160p|4k".to_string()
    } else {
        format!("(?i){}", regex::escape(quality))
    };
    Regex::new(&pattern).map(|r| r.is_match(title)).unwrap_or(true)
}

async fn fetch_detail(client: &reqwest::Client, view_url: &str) -> Result<(String, Option<String>), String> {
    let html = client
        .get(view_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let doc = Html::parse_document(&html);

    let desc_sel = Selector::parse("#torrent-description").unwrap();
    let line_sel = Selector::parse("#torrent-description li, #torrent-description p").unwrap();
    let lines: Vec<String> = doc
        .select(&line_sel)
        .map(|el| el.text().collect::<Vec<_>>().join(""))
        .collect();
    let description = if !lines.is_empty() {
        lines.join("\n")
    } else {
        doc.select(&desc_sel)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    };

    let magnet_sel = Selector::parse("a[href^='magnet:']").unwrap();
    let magnet = doc
        .select(&magnet_sel)
        .next()
        .and_then(|el| el.value().attr("href"))
        .map(|s| s.to_string());

    Ok((description, magnet))
}

fn extract_language_line(description: &str, label_pattern: &str) -> Option<String> {
    let re = Regex::new(&format!(
        r"(?im)^(?:[-*][ \t]+)?[`*]{{0,2}}{label_pattern}s?[ \t]*(?:\(\d+\))?[`*]{{0,2}}:?[ \t]*(.*)$"
    ))
    .ok()?;
    let caps = re.captures(description)?;
    let inline = caps[1].trim();
    let value = if !inline.is_empty() {
        inline.to_string()
    } else {
        let rest = &description[caps.get(0)?.end()..];
        rest.lines()
            .skip_while(|l| l.trim().is_empty())
            .take_while(|l| {
                let l = l.trim();
                !l.is_empty() && !l.starts_with("**") && !l.starts_with('#')
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    if value.is_empty() {
        return None;
    }
    Some(value.replace('*', ""))
}

const LANGUAGE_ALIASES: &[(&str, &[&str])] = &[
    ("Portuguese (Brazil)", &["portuguese (brazilian)"]),
    ("Spanish (Latin America)", &["spanish (latin american)"]),
    ("Spanish (Spain)", &["spanish (european)", "spanish (castilian)"]),
    ("Chinese (Simplified)", &["chinese (china)", "chinese (mainland)"]),
    ("Chinese (Traditional)", &["chinese (hong kong)", "chinese (taiwan)"]),
];

fn language_matches(line_lower: &str, canonical: &str) -> bool {
    if line_lower.contains(&canonical.to_lowercase()) {
        return true;
    }
    LANGUAGE_ALIASES
        .iter()
        .find(|(name, _)| *name == canonical)
        .is_some_and(|(_, aliases)| aliases.iter().any(|a| line_lower.contains(a)))
}

/// (language, search tag, lowercase markers in release titles). Releases in other
/// languages usually state it in the title and sit in the "non-English" category.
const TITLE_TAGS: &[(&str, &str, &[&str])] = &[
    ("Portuguese (Brazil)", "PT-BR", &["pt-br", "ptbr", "pt br", "legendado", "dublado", "português"]),
    ("Spanish (Latin America)", "Latino", &["latino", "esp-lat", "spa-lat", "español latino"]),
    ("Spanish (Spain)", "Castellano", &["castellano", "esp-es", "español españa"]),
    ("French", "VOSTFR", &["vostfr", "subfrench", "french"]),
    ("German", "German", &["german", "gersub", "deutsch"]),
    ("Italian", "ITA", &["[ita]", "sub ita", "italian"]),
    ("Russian", "RUS", &["[rus]", "russian"]),
    ("Arabic", "Arabic", &["arabic"]),
    ("Chinese (Simplified)", "CHS", &["chs", "简"]),
    ("Chinese (Traditional)", "CHT", &["cht", "繁"]),
    ("Polish", "PL", &["polish", "napisy pl"]),
    ("Indonesian", "Indonesia", &["indonesia", "subindo"]),
    ("Malay", "Malay", &["malay"]),
    ("Thai", "Thai", &["thai"]),
    ("Vietnamese", "Vietsub", &["vietsub", "vietnamese"]),
];
const DUB_MARKERS: [&str; 6] = ["dub", "dublado", "dual", "doblaje", "multi-audio", "multi audio"];

fn title_tags(language: &str) -> Option<&'static (&'static str, &'static str, &'static [&'static str])> {
    TITLE_TAGS.iter().find(|(name, _, _)| *name == language)
}

fn title_has_language(title_lower: &str, language: &str) -> bool {
    title_tags(language).is_some_and(|(_, _, markers)| markers.iter().any(|m| title_lower.contains(m)))
}

/// Extra searches for anime filtered by a non-English language: the non-English
/// category, and the query with the language tag across all anime.
fn language_searches(query: &str, audio_langs: &[String], sub_langs: &[String]) -> Vec<(String, &'static str)> {
    let tags: Vec<&str> = audio_langs
        .iter()
        .chain(sub_langs)
        .filter_map(|l| title_tags(l).map(|(_, tag, _)| *tag))
        .collect();
    if tags.is_empty() {
        return Vec::new();
    }
    let mut out = vec![(query.to_string(), "1_3")];
    for tag in tags {
        let q = format!("{query} {tag}");
        if !out.iter().any(|(existing, _)| *existing == q) {
            out.push((q, "1_0"));
        }
    }
    out
}

async fn with_language_searches(
    client: &reqwest::Client,
    query: &str,
    audio_langs: &[String],
    sub_langs: &[String],
    mut base: Vec<NyaaCandidate>,
) -> Vec<NyaaCandidate> {
    let mut seen: HashSet<String> = base.iter().map(|c| c.id.clone()).collect();
    for (q, category) in language_searches(query, audio_langs, sub_langs) {
        if let Ok(found) = search_in(client, &q, category).await {
            base.extend(found.into_iter().filter(|c| seen.insert(c.id.clone())));
        }
    }
    base
}

/// Language filters satisfied by the release page's description or, failing that,
/// by tags in the title ("[PT-BR]", "Legendado", "Dublado").
pub fn matches_language(description: &str, title: &str, audio_langs: &[String], sub_langs: &[String]) -> bool {
    let title = title.to_lowercase();
    let dubbed = DUB_MARKERS.iter().any(|m| title.contains(m));
    let audio_ok = audio_langs.is_empty()
        || matches_language_text(description, audio_langs, &[])
        || (dubbed && audio_langs.iter().any(|l| title_has_language(&title, l)));
    let sub_ok = sub_langs.is_empty()
        || matches_language_text(description, &[], sub_langs)
        || sub_langs.iter().any(|l| title_has_language(&title, l));
    audio_ok && sub_ok
}

pub fn matches_language_text(description: &str, audio_langs: &[String], sub_langs: &[String]) -> bool {
    let audio_ok = audio_langs.is_empty()
        || extract_language_line(description, "Audio:?")
            .map(|line| {
                let lower = line.to_lowercase();
                audio_langs.iter().any(|l| language_matches(&lower, l))
            })
            .unwrap_or(false);

    let sub_ok = sub_langs.is_empty()
        || extract_language_line(description, "Subtitles?:?")
            .map(|line| {
                let lower = line.to_lowercase();
                sub_langs.iter().any(|l| language_matches(&lower, l))
            })
            .unwrap_or(false);

    audio_ok && sub_ok
}

const KNOWN_LANGUAGES: &[&str] = &[
    "Japanese",
    "English",
    "Portuguese (Brazil)",
    "Spanish (Latin America)",
    "Spanish (Spain)",
    "French",
    "German",
    "Italian",
    "Russian",
    "Arabic",
    "Chinese (Simplified)",
    "Chinese (Traditional)",
    "Polish",
    "Indonesian",
    "Malay",
    "Thai",
    "Vietnamese",
];

const LANGUAGE_SAMPLE_SIZE: usize = 10;

#[derive(Debug, Serialize)]
pub struct AvailableLanguages {
    pub audio: Vec<String>,
    pub subtitles: Vec<String>,
}

pub async fn list_available_languages(client: &reqwest::Client, query: &str) -> Result<AvailableLanguages, String> {
    let (search_query, _season) = split_season(query);
    let all = search(client, &search_query).await?;
    let sample: Vec<_> = all.into_iter().take(LANGUAGE_SAMPLE_SIZE).collect();

    let descriptions: Vec<String> = stream::iter(sample)
        .map(|candidate| {
            let client = client.clone();
            async move {
                fetch_detail(&client, &candidate.view_url)
                    .await
                    .map(|(description, _)| description)
                    .unwrap_or_default()
            }
        })
        .buffer_unordered(DETAIL_FETCH_CONCURRENCY)
        .collect()
        .await;

    Ok(aggregate_languages(&descriptions))
}

fn aggregate_languages(descriptions: &[String]) -> AvailableLanguages {
    let mut audio = HashSet::new();
    let mut subtitles = HashSet::new();
    for description in descriptions {
        if let Some(line) = extract_language_line(description, "Audio:?") {
            let lower = line.to_lowercase();
            audio.extend(KNOWN_LANGUAGES.iter().filter(|l| language_matches(&lower, l)));
        }
        if let Some(line) = extract_language_line(description, "Subtitles?:?") {
            let lower = line.to_lowercase();
            subtitles.extend(KNOWN_LANGUAGES.iter().filter(|l| language_matches(&lower, l)));
        }
    }

    let sort_key = |s: &&str| KNOWN_LANGUAGES.iter().position(|k| k == s).unwrap_or(usize::MAX);
    let mut audio: Vec<&str> = audio.into_iter().collect();
    audio.sort_by_key(sort_key);
    let mut subtitles: Vec<&str> = subtitles.into_iter().collect();
    subtitles.sort_by_key(sort_key);

    AvailableLanguages {
        audio: audio.into_iter().map(String::from).collect(),
        subtitles: subtitles.into_iter().map(String::from).collect(),
    }
}

fn group_best_per_episode(candidates: Vec<NyaaCandidate>) -> Vec<EpisodeMatch> {
    let mut groups: Vec<EpisodeMatch> = Vec::new();
    let mut episode_index: HashMap<u32, usize> = HashMap::new();

    for candidate in candidates {
        match extract_episode_number(&candidate.title) {
            Some(ep) => match episode_index.get(&ep) {
                Some(&idx) => groups[idx].alternates.push(candidate),
                None => {
                    episode_index.insert(ep, groups.len());
                    groups.push(EpisodeMatch { primary: candidate, alternates: Vec::new() });
                }
            },
            None => groups.push(EpisodeMatch { primary: candidate, alternates: Vec::new() }),
        }
    }

    for group in &mut groups {
        if group.alternates.is_empty() {
            continue;
        }
        let mut all = std::mem::take(&mut group.alternates);
        all.push(group.primary.clone());
        all.sort_by_key(|c| std::cmp::Reverse(c.seeders.unwrap_or(0)));
        group.primary = all.remove(0);
        group.alternates = all;
    }

    groups
}

#[allow(clippy::too_many_arguments)]
pub async fn find_new_matches(
    client: &reqwest::Client,
    query: &str,
    names: &[String],
    quality: &str,
    audio_langs: &[String],
    sub_langs: &[String],
    episode_start: Option<i64>,
    episode_end: Option<i64>,
    seen_ids: &HashSet<String>,
) -> Result<MatchResult, String> {
    let (search_query, season) = split_season(query);
    let all = search(client, &search_query).await?;
    let all = match season {
        Some(s) => search_with_episode_probes(client, &search_query, s, episode_start, episode_end, all).await,
        None => all,
    };
    let all = with_language_searches(client, &search_query, audio_langs, sub_langs, all).await;
    let mut rejected = Rejections::default();
    let mut new_candidates = Vec::new();
    for c in all.into_iter().filter(|c| !seen_ids.contains(&c.id)) {
        if !matches_quality(&c.title, quality) {
            rejected.quality += 1;
        } else if season.is_some_and(|s| !title_matches_season(&c.title, s)) {
            rejected.season += 1;
        } else if !title_matches_episode_range(&c.title, episode_start, episode_end) {
            rejected.range += 1;
        } else if is_other_work(&c.title, names, season.unwrap_or(1)) {
            rejected.other_work += 1;
        } else {
            new_candidates.push(c);
        }
    }

    let finish = |passed: Vec<NyaaCandidate>, all_new: Vec<NyaaCandidate>, rejected: Rejections| {
        let (numbered, batches): (Vec<_>, Vec<_>) =
            passed.into_iter().partition(|c| extract_episode_number(&c.title).is_some());
        MatchResult { matched: group_best_per_episode(numbered), all_new, batches, rejected }
    };

    let need_lang_check = !audio_langs.is_empty() || !sub_langs.is_empty();
    if !need_lang_check {
        return Ok(finish(new_candidates.clone(), new_candidates, rejected));
    }

    let audio_langs = audio_langs.to_vec();
    let sub_langs = sub_langs.to_vec();

    let matched: Vec<NyaaCandidate> = stream::iter(new_candidates.clone())
        .map(|mut candidate| {
            let client = client.clone();
            let audio_langs = audio_langs.clone();
            let sub_langs = sub_langs.clone();
            async move {
                match fetch_detail(&client, &candidate.view_url).await {
                    Ok((description, real_magnet)) => {
                        if let Some(m) = real_magnet {
                            candidate.magnet = m;
                        }
                        matches_language(&description, &candidate.title, &audio_langs, &sub_langs)
                            .then_some(candidate)
                    }
                    Err(_) => None,
                }
            }
        })
        .buffer_unordered(DETAIL_FETCH_CONCURRENCY)
        .filter_map(|x| async move { x })
        .collect()
        .await;

    rejected.language = new_candidates.len() - matched.len();
    Ok(finish(matched, new_candidates, rejected))
}

pub async fn find_best_for_episode(
    client: &reqwest::Client,
    query: &str,
    quality: &str,
    audio_langs: &[String],
    sub_langs: &[String],
    episode_number: u32,
) -> Result<Option<EpisodeMatch>, String> {
    let (base_query, season) = split_season(query);
    let season = season.unwrap_or(1);
    let probe_query = format!("{base_query} S{season:02}E{episode_number:02}");
    let candidates = search(client, &probe_query).await?;
    let candidates = with_language_searches(client, &base_query, audio_langs, sub_langs, candidates).await;

    let filtered: Vec<NyaaCandidate> = candidates
        .into_iter()
        .filter(|c| matches_quality(&c.title, quality))
        .filter(|c| extract_episode_number(&c.title) == Some(episode_number))
        .collect();

    let need_lang_check = !audio_langs.is_empty() || !sub_langs.is_empty();
    let matched = if !need_lang_check {
        filtered
    } else {
        let audio_langs = audio_langs.to_vec();
        let sub_langs = sub_langs.to_vec();
        stream::iter(filtered)
            .map(|mut candidate| {
                let client = client.clone();
                let audio_langs = audio_langs.clone();
                let sub_langs = sub_langs.clone();
                async move {
                    match fetch_detail(&client, &candidate.view_url).await {
                        Ok((description, real_magnet)) => {
                            if let Some(m) = real_magnet {
                                candidate.magnet = m;
                            }
                            matches_language(&description, &candidate.title, &audio_langs, &sub_langs).then_some(candidate)
                        }
                        Err(_) => None,
                    }
                }
            })
            .buffer_unordered(DETAIL_FETCH_CONCURRENCY)
            .filter_map(|x| async move { x })
            .collect()
            .await
    };

    Ok(group_best_per_episode(matched).into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn language_from_release_title() {
        let ptbr = vec!["Portuguese (Brazil)".to_string()];
        assert!(matches_language("", "Steins;Gate (01-24) [1080p] [PT-BR]", &[], &ptbr));
        assert!(matches_language("", "[Punch-Fansub] Steins;Gate: Episódio 18 HD sub pt-br", &[], &ptbr));
        assert!(!matches_language("", "[Cleo] Steins;Gate [Dual Audio 10bit BD1080p]", &[], &ptbr));
        assert!(!matches_language("", "Show - 05 [PT-BR]", &ptbr, &[]));
        assert!(matches_language("", "Show - 05 [Dublado PT-BR]", &ptbr, &[]));
        assert!(language_searches("Show", &[], &[]).is_empty());
        assert_eq!(
            language_searches("Show", &[], &ptbr),
            vec![("Show".to_string(), "1_3"), ("Show PT-BR".to_string(), "1_0")]
        );
    }

    #[test]
    fn episode_number_from_common_anime_naming() {
        assert_eq!(extract_episode_number("[SubsPlease] Show - 05 (1080p) [ABCD].mkv"), Some(5));
        assert_eq!(extract_episode_number("[Group] Show - 12v2 [1080p]"), Some(12));
        assert_eq!(extract_episode_number("Show - 07.mkv"), Some(7));
        assert_eq!(extract_episode_number("Show S02E03 1080p"), Some(3));
        assert_eq!(extract_episode_number("Show EP08 [720p]"), Some(8));
        assert_eq!(extract_episode_number("[Erai-raws] Show - 01 ~ 23 [1080p]"), None);
        assert_eq!(extract_episode_number("[HorribleSubs] Show (01-24) [720p] (Batch)"), None);
    }

    #[test]
    fn other_works_of_the_franchise_are_rejected() {
        let sg = names(&["Steins;Gate"]);
        assert!(is_other_work("[HorribleSubs] Steins Gate 0 - 24 [1080p].mkv", &sg, 1));
        assert!(is_other_work("[Exiled-Destiny] Steins;Gate - The Movie [Dual Audio]", &sg, 1));
        assert!(!is_other_work("[Cleo] Steins;Gate [Dual Audio 10bit BD1080p][HEVC-x265]", &sg, 1));
        assert!(!is_other_work("[Group] Steins;Gate - 05 [1080p]", &sg, 1));
        let sg0 = names(&["Steins;Gate 0", "Steins;Gate"]);
        assert!(!is_other_work("[HorribleSubs] Steins Gate 0 - 24 [1080p].mkv", &sg0, 1));
        let show = names(&["Show"]);
        assert!(is_other_work("[Group] Show Season 2 - 05", &show, 1));
        assert!(!is_other_work("[Group] Show Season 2 - 05", &show, 2));
        assert!(is_other_work("[Group] Show 2nd Season - 05", &show, 1));
        assert!(!is_other_work("[Group] Unrelated Name - 05", &show, 1));
        assert!(is_other_work("Gekijouban Steins;Gate - Fuka Ryouiki no Deja vu", &sg, 1));
    }

    #[test]
    fn pack_files_map_to_episodes() {
        let sg = names(&["Steins;Gate"]);
        let root = "[Judas] Steins;Gate (Season 1 + OVA + Movie)";
        assert_eq!(pack_file_episode(&format!("{root}/[Judas] Steins;Gate - 05.mkv"), &sg, 1), Some(5));
        assert_eq!(pack_file_episode(&format!("{root}/Specials/[Judas] Steins;Gate - OVA.mkv"), &sg, 1), None);
        assert_eq!(pack_file_episode(&format!("{root}/NCOP/Steins;Gate NCOP 01.mkv"), &sg, 1), None);
        assert_eq!(pack_file_episode(&format!("{root}/[Judas] Steins;Gate - 05.ass"), &sg, 1), None);
        assert_eq!(pack_file_episode("Steins;Gate 0/[Group] Steins;Gate 0 - 05.mkv", &sg, 1), None);
        assert_eq!(pack_file_episode("Show/12 - Title.mkv", &names(&["Show"]), 1), Some(12));
        assert_eq!(pack_file_episode("Show S02E03.mkv", &names(&["Show"]), 1), None);
    }

    const SEARCH_FIXTURE: &str = include_str!("../../tests/fixtures/nyaa_search.xml");

    #[test]
    fn parses_real_rss_fixture() {
        let items = parse_rss(SEARCH_FIXTURE).expect("should parse");
        assert!(!items.is_empty(), "fixture should contain at least one item");

        let first = &items[0];
        assert!(!first.id.is_empty());
        assert!(!first.title.is_empty());
        assert!(
            first.magnet.starts_with("magnet:?xt=urn:btih:"),
            "magnet should be built from infoHash: {}",
            first.magnet
        );
        assert!(first.view_url.starts_with("https://nyaa.si/view/"));
    }

    #[test]
    fn episode_probe_queries_covers_full_range_with_zero_padded_episode() {
        let queries = episode_probe_queries("Mushoku Tensei: Jobless Reincarnation", 3, None, None);
        assert_eq!(queries.len(), EPISODE_PROBE_MAX as usize);
        assert_eq!(queries[0], "Mushoku Tensei: Jobless Reincarnation S03E01");
        assert_eq!(
            queries[23],
            "Mushoku Tensei: Jobless Reincarnation S03E24"
        );
    }

    #[test]
    fn split_season_strips_trailing_season_and_returns_number() {
        assert_eq!(
            split_season("Mushoku Tensei: Jobless Reincarnation Season 3"),
            ("Mushoku Tensei: Jobless Reincarnation".to_string(), Some(3))
        );
        assert_eq!(
            split_season("Some Show 3rd Season"),
            ("Some Show".to_string(), Some(3))
        );
        assert_eq!(
            split_season("Show With No Season"),
            ("Show With No Season".to_string(), None)
        );
    }

    #[test]
    fn sanitize_query_defuses_dash_search_syntax() {
        assert_eq!(
            sanitize_query("Re:ZERO -Starting Life in Another World- S04E01"),
            "Re:ZERO  Starting Life in Another World  S04E01"
        );
    }

    #[test]
    fn episode_probe_queries_respects_configured_range() {
        let queries = episode_probe_queries("Show", 3, Some(5), Some(7));
        assert_eq!(queries, vec!["Show S03E05", "Show S03E06", "Show S03E07"]);
    }

    #[test]
    fn episode_probe_queries_open_ended_range_clamps_to_max() {
        let queries = episode_probe_queries("Show", 3, Some(EPISODE_PROBE_MAX as i64 - 1), None);
        assert_eq!(queries.len(), 2);
    }

    #[test]
    fn episode_probe_queries_empty_when_start_after_end() {
        assert!(episode_probe_queries("Show", 3, Some(10), Some(5)).is_empty());
    }

    #[test]
    fn title_matches_episode_range_filters_by_extracted_episode_number() {
        assert!(title_matches_episode_range("Show S03E05 Title", Some(5), Some(10)));
        assert!(!title_matches_episode_range("Show S03E04 Title", Some(5), Some(10)));
        assert!(!title_matches_episode_range("Show S03E11 Title", Some(5), Some(10)));
        assert!(title_matches_episode_range("Show S03E01 Title", None, None));
        assert!(title_matches_episode_range("Show Batch Complete", Some(5), Some(10)));
        assert!(title_matches_episode_range("Show S03E99 Title", Some(5), None));
        assert!(!title_matches_episode_range("Show S03E02 Title", Some(5), None));
    }

    fn candidate(id: &str, title: &str) -> NyaaCandidate {
        NyaaCandidate {
            id: id.to_string(),
            title: title.to_string(),
            magnet: String::new(),
            view_url: String::new(),
            size: None,
            seeders: None,
            leechers: None,
            published_at: None,
        }
    }

    fn candidate_with_seeders(id: &str, title: &str, seeders: i32) -> NyaaCandidate {
        NyaaCandidate { seeders: Some(seeders), ..candidate(id, title) }
    }

    #[test]
    fn group_best_per_episode_picks_most_seeded_release_as_primary() {
        let candidates = vec![
            candidate_with_seeders("1", "[Feibanyama] Show S04E01 [2160p]", 3),
            candidate_with_seeders("2", "[ToonsHub] Show S04E01 [1080p]", 40),
            candidate_with_seeders("3", "[Feibanyama] Show S04E02 [2160p]", 12),
            candidate_with_seeders("4", "[ToonsHub] Show S04E02 [1080p]", 5),
        ];
        let result = group_best_per_episode(candidates);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].primary.id, "2");
        assert_eq!(result[0].alternates.len(), 1);
        assert_eq!(result[0].alternates[0].id, "1");
        assert_eq!(result[1].primary.id, "3");
        assert_eq!(result[1].alternates[0].id, "4");
    }

    #[test]
    fn group_best_per_episode_passes_through_titles_without_recognizable_episode_number() {
        let candidates = vec![candidate("1", "Show Batch Complete"), candidate("2", "Show Batch Complete v2")];
        let result = group_best_per_episode(candidates);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|g| g.alternates.is_empty()));
    }

    #[test]
    fn group_best_per_episode_missing_seeder_count_treated_as_zero() {
        let candidates = vec![
            candidate("1", "[NoSeedInfo] Show S04E01 [1080p]"),
            candidate_with_seeders("2", "[ToonsHub] Show S04E01 [1080p]", 1),
        ];
        let result = group_best_per_episode(candidates);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].primary.id, "2");
    }

    #[test]
    fn title_matches_season_covers_real_world_naming_conventions() {
        assert!(title_matches_season(
            "[Yameii] Mushoku Tensei: Jobless Reincarnation - S03E11 [English Dub]",
            3
        ));
        assert!(title_matches_season(
            "[Doomdos] - Mushoku Tensei Jobless Reincarnation Season 3 - 13 [1080p IQ WEB-DL]",
            3
        ));
        assert!(title_matches_season(
            "Mushoku Tensei Jobless Reincarnation S03E10",
            3
        ));
        assert!(title_matches_season(
            "[Fuchs] Mushoku Tensei - S03E05 ... | Jobless Reincarnation (Season 3)",
            3
        ));
        assert!(!title_matches_season(
            "[ZeroBuild] Mushoku Tensei: Jobless Reincarnation Season 2 Cour 2",
            3
        ));
        assert!(!title_matches_season(
            "[sam] Mushoku Tensei Jobless Reincarnation Season 2 (S02) Part 1",
            3
        ));
    }

    #[test]
    fn quality_filter_matches_case_insensitively() {
        assert!(matches_quality("[Group] Show - 01 [1080p][AAC]", "1080p"));
        assert!(matches_quality("[Group] Show - 01 [1080P][AAC]", "1080p"));
        assert!(!matches_quality("[Group] Show - 01 [720p][AAC]", "1080p"));
        assert!(!matches_quality("[Group] Show 1080 - 01", "1080p"));
    }

    #[test]
    fn quality_filter_2160p_accepts_4k_alias() {
        assert!(matches_quality("[Group] Show - 01 [2160p]", "2160p"));
        assert!(matches_quality("[Group] Show - 01 [4K][HEVC]", "2160p"));
    }

    #[test]
    fn quality_filter_any_matches_everything() {
        assert!(matches_quality("[Group] Show - 01 [480p]", "any"));
        assert!(matches_quality("[Group] Show - 01 [480p]", ""));
    }

    #[test]
    fn language_line_extraction_tolerates_bold_markdown() {
        let description = "Some notes\n**Audio:** Japanese\n**Subtitles:** English, Portuguese (Brazil)\n";
        let audio = extract_language_line(description, "Audio:?").unwrap();
        assert_eq!(audio, "Japanese");
        let subs = extract_language_line(description, "Subtitles?:?").unwrap();
        assert_eq!(subs, "English, Portuguese (Brazil)");
    }

    #[test]
    fn language_line_extraction_reads_block_lists() {
        let description = "**Audio**\nJapanese / E-AC-3 / 2.0\nEnglish / E-AC-3 / 2.0\n\n\
            **Subtitles**\nEnglish / Full / Default / ASS\nPortuguese (Brazil) / Full / Default / ASS / CR\n\n\
            **Chapters**\nPrologue / Opening / Part A";
        let subs = extract_language_line(description, "Subtitles?:?").unwrap();
        assert!(subs.contains("Portuguese (Brazil)"));
        assert!(!subs.contains("Prologue"));
        let audio = extract_language_line(description, "Audio:?").unwrap();
        assert!(audio.contains("English") && !audio.contains("Portuguese"));
        assert!(matches_language_text(description, &[], &["Portuguese (Brazil)".to_string()]));
    }

    #[test]
    fn matches_language_text_checks_any_of_the_wanted_languages() {
        let description = "Audio: Japanese\nSubtitles: English, Portuguese (Brazil)";
        assert!(matches_language_text(
            description,
            &["Japanese".to_string()],
            &["Portuguese (Brazil)".to_string()]
        ));
        assert!(!matches_language_text(
            description,
            &["French".to_string()],
            &[]
        ));
        assert!(matches_language_text(description, &[], &[]));
    }

    #[test]
    fn matches_language_text_ignores_bold_markdown_around_each_language_name() {
        let description = "`Subtitles (15):` **English** [Forced], ASS │ **Portuguese** (Brazilian), ASS │ **Russian**, ASS";
        assert!(matches_language_text(
            description,
            &[],
            &["Portuguese (Brazil)".to_string()]
        ));
    }

    fn extract_description(html: &str) -> String {
        let doc = Html::parse_document(html);
        let desc_sel = Selector::parse("#torrent-description").unwrap();
        let line_sel = Selector::parse("#torrent-description li, #torrent-description p").unwrap();
        let lines: Vec<String> = doc
            .select(&line_sel)
            .map(|el| el.text().collect::<Vec<_>>().join(""))
            .collect();
        if !lines.is_empty() {
            lines.join("\n")
        } else {
            doc.select(&desc_sel)
                .next()
                .map(|el| el.text().collect::<Vec<_>>().join("\n"))
                .unwrap_or_default()
        }
    }

    const REAL_DETAIL_FIXTURE: &str = include_str!("../../tests/fixtures/nyaa_detail_realworld.html");
    const REAL_VARYG_FIXTURE: &str = include_str!("../../tests/fixtures/nyaa_detail_varyg.html");

    #[test]
    fn fetch_detail_matches_real_world_varyg_page() {
        let description = extract_description(REAL_VARYG_FIXTURE);
        assert!(matches_language_text(
            &description,
            &[],
            &["Portuguese (Brazil)".to_string()]
        ));
        assert!(!matches_language_text(
            &description,
            &[],
            &["Klingon".to_string()]
        ));
    }

    #[test]
    fn fetch_detail_matches_real_world_toonshub_page() {
        let description = extract_description(REAL_DETAIL_FIXTURE);
        assert!(matches_language_text(
            &description,
            &["Portuguese (Brazil)".to_string()],
            &[]
        ));
        assert!(!matches_language_text(
            &description,
            &["Klingon".to_string()],
            &[]
        ));
    }

    #[test]
    fn aggregate_languages_extracts_known_languages_from_real_page() {
        let description = extract_description(REAL_DETAIL_FIXTURE);
        let result = aggregate_languages(&[description]);
        assert_eq!(
            result.audio,
            vec![
                "Japanese",
                "English",
                "Portuguese (Brazil)",
                "Spanish (Latin America)",
                "Spanish (Spain)",
                "French",
                "German",
                "Italian",
            ]
        );
        assert!(result.subtitles.contains(&"Portuguese (Brazil)".to_string()));
        assert!(result.subtitles.contains(&"Thai".to_string()));
        assert!(!result.audio.iter().any(|l| l == "Vietnamese" || l == "Polish"));
    }

    #[test]
    fn aggregate_languages_dedupes_across_multiple_descriptions() {
        let a = "Audio: Japanese, English".to_string();
        let b = "Audio: English, French".to_string();
        let result = aggregate_languages(&[a, b]);
        assert_eq!(result.audio, vec!["Japanese", "English", "French"]);
    }

    #[test]
    fn fetch_detail_handles_raw_markdown_blob_with_bullet_dashes() {
        let html = "<html><body><div id=\"torrent-description\">**File Details:**&#10;\
            - **Video Quality:** 1080p WEB-DL H.264 (CR)&#10;\
            - **Audio:** Japanese, English, Portuguese (Brazil)&#10;\
            - **Subtitles:** English, Portuguese (Brazil)&#10;\
            </div></body></html>";

        let description = extract_description(html);
        assert!(matches_language_text(
            &description,
            &["Portuguese (Brazil)".to_string()],
            &["Portuguese (Brazil)".to_string()]
        ));
    }

    #[test]
    fn fetch_detail_merges_label_and_value_split_across_rendered_tags() {
        let html = r#"
            <html><body>
              <div id="torrent-description">
                <p><strong>File Details:</strong></p>
                <ul>
                  <li><strong>Video Quality:</strong> 1080p WEB-DL H.264 (CR)</li>
                  <li><strong>Audio:</strong> Japanese, English, Portuguese (Brazil)</li>
                  <li><strong>Subtitles:</strong> English, Portuguese (Brazil)</li>
                </ul>
              </div>
            </body></html>
        "#;

        let description = extract_description(html);
        assert!(matches_language_text(
            &description,
            &["Portuguese (Brazil)".to_string()],
            &[]
        ));
    }

    #[test]
    fn matches_language_text_missing_line_fails_closed() {
        let description = "No language info here.";
        assert!(!matches_language_text(
            description,
            &["Japanese".to_string()],
            &[]
        ));
    }
}
