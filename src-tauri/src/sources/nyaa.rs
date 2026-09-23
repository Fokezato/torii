use futures::stream::{self, StreamExt};
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const BASE_URL: &str = "https://nyaa.si";
/// Categoria 1_2 = "Anime - English-translated", onde ficam os releases fansub/CR com áudio/legenda multi-idioma.
const CATEGORY: &str = "1_2";
const DETAIL_FETCH_CONCURRENCY: usize = 3;
/// Pra títulos populares com muito reupload (ex. Mushoku Tensei), a busca do
/// nyaa por nome+season sozinha estoura a página 1 (~75 itens) sem cobrir
/// episódios mais antigos da season — o ranking dele não é confiável pra
/// esse caso. Busca "<query> S0NE0M" por episódio é cirúrgica (achou exato
/// com poucos itens em teste manual), então sonda um range de episódios e
/// mescla com a busca ampla. 24 cobre a esmagadora maioria das seasons.
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
}

/// Um episódio com 2+ releases casando vira 1 `EpisodeMatch`: `primary` (mais
/// seeders — não repack/grupo específico, só a fonte mais saudável pra
/// baixar) e `alternates` (o resto, guardado em `episode_sources` pro
/// usuário trocar manualmente depois via menu "..." na Biblioteca).
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

/// O nyaa trata hífen colado numa palavra (início: "-palavra" vira operador
/// de exclusão tipo Google; fim: "palavra-" quebra o casamento de token) de
/// forma especial nos dois casos — testado contra a API de verdade. Títulos
/// estilizados tipo "Re:ZERO -Starting Life in Another World- Season 4" têm
/// hífen decorativo exatamente nessas posições e a busca não achava nada,
/// mesmo o torrent existindo (https://nyaa.si/view/2095563). Mais simples
/// trocar todo hífen por espaço na query — não precisa ser bonito, só achar.
fn sanitize_query(query: &str) -> String {
    query.replace('-', " ")
}

pub async fn search(client: &reqwest::Client, query: &str) -> Result<Vec<NyaaCandidate>, String> {
    let query = sanitize_query(query);
    let url = format!(
        "{BASE_URL}/?page=rss&c={CATEGORY}&q={}",
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

/// Sem range configurado, sonda 1..=EPISODE_PROBE_MAX (cobre a esmagadora
/// maioria das seasons). Com range, sonda só o que interessa — mais rápido
/// e evita trazer de volta episódios fora do que o usuário pediu.
fn episode_probe_queries(base_query: &str, season: u32, start: Option<i64>, end: Option<i64>) -> Vec<String> {
    let lo = start.unwrap_or(1).max(1) as u32;
    let hi = end.unwrap_or(EPISODE_PROBE_MAX as i64).clamp(1, EPISODE_PROBE_MAX as i64) as u32;
    if lo > hi {
        return Vec::new();
    }
    (lo..=hi).map(|ep| format!("{base_query} S{season:02}E{ep:02}")).collect()
}

/// Busca "<query> S0NE01", "...E02", etc. em paralelo e mescla com `base`,
/// removendo duplicata por id. Ver comentário de `EPISODE_PROBE_MAX`.
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

/// O nyaa faz busca tipo AND por palavra: pedir "Season 3" literal exclui
/// releases que abreviam como "S03E10" (sem a palavra "Season" no título).
/// Tira o sufixo de season da query pra buscar mais amplo, devolvendo o
/// número pra filtrar depois via `title_matches_season`.
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

/// Casa "S03", "S3", "S03E10" ou "Season 3" no título do candidato. Sem
/// isso, buscar com a query ampliada (via `split_season`) traria de volta
/// releases de outras seasons do mesmo anime.
fn title_matches_season(title: &str, season: u32) -> bool {
    let pattern = format!(r"(?i)\bs0*{season}(?:e\d+)?\b|\bseason\s*0*{season}\b");
    Regex::new(&pattern).map(|r| r.is_match(title)).unwrap_or(true)
}

pub fn extract_episode_number(title: &str) -> Option<u32> {
    let re = Regex::new(r"(?i)S\d{1,2}E(\d{1,3})").ok()?;
    re.captures(title)?.get(1)?.as_str().parse().ok()
}

/// Filtro "avançado" de range de episódio (ex. baixar só do 5 ao 10). Sem
/// range configurado (ambos None) sempre casa. Título sem "SxxEyy"
/// reconhecível também deixa passar — os outros filtros (season/qualidade/
/// idioma) continuam se aplicando de qualquer forma.
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

/// "any"/vazio sempre casa. 2160p também aceita a tag "4k", comum em releases.
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

    // Uploaders variam: alguns (ex. ToonsHub) mandam markdown cru num único
    // text node (`&#10;` vira quebra de linha real ao decodificar entidades);
    // outros renderizam <li>/<p> de verdade, o que fragmenta label e valor em
    // text nodes separados (<strong>Audio:</strong> vira um node, o valor
    // depois vira outro). Junta por li/p quando eles existem; senão cai pro
    // texto bruto do container, que já preserva as linhas via \n decodificado.
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
    // "(?:[-*]\s+)?" tolera o bullet markdown ("- " ou "* ") que antecede o
    // label em listas cruas tipo "- **Audio:** Japanese, ...". Uploaders no
    // estilo MediaInfo (VARYG, EMBER) envolvem o label em backtick em vez de
    // negrito e adicionam a contagem entre parênteses antes do ":"
    // ("`Subtitles (15):`", "`Audios (2):`") — "s?\s*(?:\(\d+\))?" cobre o
    // plural + contagem, e a classe de caractere aceita "`" além de "*".
    let re = Regex::new(&format!(
        r"(?im)^(?:[-*]\s+)?[`*]{{0,2}}{label_pattern}s?\s*(?:\(\d+\))?[`*]{{0,2}}:?\s*(.+)$"
    ))
    .ok()?;
    // Cada idioma da lista também costuma vir em negrito ("**Portuguese**
    // (Brazilian), ASS") — sem tirar os "**" daqui, o "**" entre o nome e o
    // "(Brazilian)" quebra o match de substring contra "portuguese (brazil)"
    // em `matches_language_text`, mesmo o idioma estando ali de verdade.
    re.captures(description)
        .map(|c| c[1].trim().replace('*', ""))
}

/// Uploaders no estilo MediaInfo (VARYG, EMBER, etc.) descrevem o idioma com
/// o adjetivo do país em vez do rótulo canônico que a UI usa — "Portuguese
/// (Brazilian)" em vez de "Portuguese (Brazil)", "Spanish (European)" em vez
/// de "Spanish (Spain)". O idioma existe de verdade no release, só o texto
/// não bate por substring direto. Cobre as variantes reais vistas até agora;
/// substring simples continua sendo o caminho principal.
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

/// Best-effort: casa contra o texto livre da descrição (convenção do uploader), igual o app antigo em Python.
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

/// Mesma lista de idiomas que o frontend oferece (src/lib/constants.ts).
/// Duplicada aqui de propósito — é só pra saber quais bater contra o texto
/// livre da descrição, não pra validar entrada do usuário.
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

/// Quantos candidatos checar pra descobrir idioma disponível. Não precisa
/// ser todo mundo — áudio/legenda disponível não muda de episódio pra
/// episódio na prática, uma amostra do topo já representa bem, e cada
/// checagem é uma fetch de página de detalhe (custo real).
const LANGUAGE_SAMPLE_SIZE: usize = 10;

#[derive(Debug, Serialize)]
pub struct AvailableLanguages {
    pub audio: Vec<String>,
    pub subtitles: Vec<String>,
}

/// Descobre quais dos `KNOWN_LANGUAGES` de fato aparecem nos releases desse
/// anime no nyaa, checando uma amostra de páginas de detalhe. Existe pra
/// não deixar o usuário escolher um idioma que nunca vai casar com nada —
/// ao invés de uma lista genérica de 17 opções, mostra só o que
/// historicamente tem torrent de verdade.
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

/// Parte pura de `list_available_languages` (sem rede), separada pra dar
/// pra testar contra fixture de verdade.
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

/// Só 1 download por número de episódio — sem isso, quando 2+ grupos lançam
/// release do mesmo episódio (ex. Feibanyama + ToonsHub, comum em títulos
/// populares) cada um vira um candidato "matched" próprio e baixa duplicado.
/// Escolhe o de mais seeders como `primary` (fonte mais saudável, não um
/// grupo específico); o resto vira `alternates`, guardado pro usuário trocar
/// manualmente depois. Título sem "SxxEyy" reconhecível (ex. batch) não tem
/// chave de grupo — vira seu próprio `EpisodeMatch` sem alternativas.
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

pub async fn find_new_matches(
    client: &reqwest::Client,
    query: &str,
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
    let new_candidates: Vec<NyaaCandidate> = all
        .into_iter()
        .filter(|c| matches_quality(&c.title, quality))
        .filter(|c| season.map_or(true, |s| title_matches_season(&c.title, s)))
        .filter(|c| title_matches_episode_range(&c.title, episode_start, episode_end))
        .filter(|c| !seen_ids.contains(&c.id))
        .collect();

    let need_lang_check = !audio_langs.is_empty() || !sub_langs.is_empty();
    if !need_lang_check {
        return Ok(MatchResult {
            matched: group_best_per_episode(new_candidates.clone()),
            all_new: new_candidates,
        });
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
                        matches_language_text(&description, &audio_langs, &sub_langs)
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

    Ok(MatchResult {
        matched: group_best_per_episode(matched),
        all_new: new_candidates,
    })
}

/// Busca cirúrgica de UM episódio, ignorando `seen_items` de propósito — é
/// pra "Forçar verificação" (menu "..." na Biblioteca) reconsiderar até
/// candidato já visto/descartado antes. Sem season detectável na query
/// (show sem "Season N" no título), assume S01 — praticamente todo release
/// do Nyaa marca "SxxEyy" mesmo pra anime de temporada única.
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
                            matches_language_text(&description, &audio_langs, &sub_langs).then_some(candidate)
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
        // Caso real: nyaa devolvia 0 resultados pra essa query mesmo o
        // torrent existindo, porque "-Starting" era lido como "exclui
        // Starting" e "World-" quebrava o casamento do token seguinte.
        // https://nyaa.si/view/2095563
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
        // sem range = sempre casa
        assert!(title_matches_episode_range("Show S03E01 Title", None, None));
        // sem numero de episodio reconhecivel = deixa passar (fail-open)
        assert!(title_matches_episode_range("Show Batch Complete", Some(5), Some(10)));
        // só start, sem end
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
        // ep1: ToonsHub tem mais seed, vira primary; Feibanyama sobra alternate.
        assert_eq!(result[0].primary.id, "2");
        assert_eq!(result[0].alternates.len(), 1);
        assert_eq!(result[0].alternates[0].id, "1");
        // ep2: Feibanyama tem mais seed dessa vez.
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
        // seeders=None (RSS às vezes não traz) não deve dar panic nem virar
        // "infinito" — trata como pior candidato possível.
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
        // Todos títulos reais puxados do nyaa.si pra "Mushoku Tensei ... Season 3".
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
        // Season 2 não pode casar quando o watch é da Season 3.
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
        // "1080" sem "p" nao deve casar (evita falso positivo com ano/resolucao truncada)
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
        // sem preferencia de idioma = sempre casa
        assert!(matches_language_text(description, &[], &[]));
    }

    #[test]
    fn matches_language_text_ignores_bold_markdown_around_each_language_name() {
        // Formato real de release VARYG/CR: cada idioma da lista vem em
        // negrito próprio, então "**Portuguese**" fica separado de
        // "(Brazilian)" pelos asteriscos — sem stripar isso o match de
        // substring falhava mesmo o PT-BR estando ali (bug real reportado).
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
        // Fixture baixada de verdade de nyaa.si/view/1957926 (Re:ZERO S03E16,
        // upload VARYG) — bug real reportado: PT-BR existe no release
        // ("`Subtitles (15):` ... **Portuguese** (Brazilian), ASS") mas não
        // batia por 2 motivos: label com backtick+contagem "(15)" não era
        // capturado, e "Brazilian" (adjetivo) não batia contra "Brazil"
        // (rótulo canônico) por substring simples.
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
        // Fixture baixada de verdade de nyaa.si/view/2164140 (Mushoku Tensei S03E10,
        // upload do ToonsHub) — o caso real que motivou esse fix.
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
        // Idioma que não existe na descrição não aparece.
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
        // Formato real do nyaa.si (ex. uploads do ToonsHub): a descrição é
        // markdown cru dentro de um único text node (`&#10;` vira \n real ao
        // decodificar), não HTML renderizado. O label vem depois de "- ".
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
        // Alguns uploaders mandam HTML de verdade já renderizado: <strong>Audio:</strong>
        // vira um text node separado do valor, então precisa juntar por <li>/<p>.
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
