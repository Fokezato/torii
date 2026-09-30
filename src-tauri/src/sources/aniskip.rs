use crate::db::skip_segments::SkipSegments;
use serde::Deserialize;

const ANILIST_API_URL: &str = "https://graphql.anilist.co";

pub async fn fetch_mal_id(client: &reqwest::Client, anilist_id: i64) -> Result<Option<i64>, String> {
    #[derive(Deserialize)]
    struct Resp {
        data: Option<Data>,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Media")]
        media: Option<Media>,
    }
    #[derive(Deserialize)]
    struct Media {
        #[serde(rename = "idMal")]
        id_mal: Option<i64>,
    }

    let body = serde_json::json!({
        "query": "query($id: Int){ Media(id: $id, type: ANIME) { idMal } }",
        "variables": { "id": anilist_id },
    });
    let resp = client
        .post(ANILIST_API_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let parsed: Resp = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.data.and_then(|d| d.media).and_then(|m| m.id_mal))
}

#[derive(Deserialize)]
struct AniSkipResponse {
    found: bool,
    #[serde(default)]
    results: Vec<AniSkipResult>,
}

#[derive(Deserialize)]
struct AniSkipResult {
    interval: AniSkipInterval,
    #[serde(rename = "skipType")]
    skip_type: String,
}

#[derive(Deserialize)]
struct AniSkipInterval {
    #[serde(rename = "startTime")]
    start_time: f64,
    #[serde(rename = "endTime")]
    end_time: f64,
}

pub async fn fetch_skip_times(
    client: &reqwest::Client,
    mal_id: i64,
    episode_number: i64,
) -> Result<SkipSegments, String> {
    let url = format!(
        "https://api.aniskip.com/v2/skip-times/{mal_id}/{episode_number}?types[]=op&types[]=ed&types[]=recap&types[]=mixed-op&types[]=mixed-ed&episodeLength=0"
    );
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(SkipSegments::default());
    }
    if !resp.status().is_success() {
        return Err(format!("AniSkip respondeu {}", resp.status()));
    }
    let parsed: AniSkipResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut segments = SkipSegments::default();
    if parsed.found {
        for r in parsed.results {
            let start_ms = (r.interval.start_time * 1000.0).round() as i64;
            let end_ms = (r.interval.end_time * 1000.0).round() as i64;
            match r.skip_type.as_str() {
                "op" => {
                    segments.intro_start_ms = Some(start_ms);
                    segments.intro_end_ms = Some(end_ms);
                    segments.intro_mixed = false;
                }
                "ed" => {
                    segments.ending_start_ms = Some(start_ms);
                    segments.ending_end_ms = Some(end_ms);
                    segments.ending_mixed = false;
                }
                "mixed-op" if segments.intro_start_ms.is_none() => {
                    segments.intro_start_ms = Some(start_ms);
                    segments.intro_end_ms = Some(end_ms);
                    segments.intro_mixed = true;
                }
                "mixed-ed" if segments.ending_start_ms.is_none() => {
                    segments.ending_start_ms = Some(start_ms);
                    segments.ending_end_ms = Some(end_ms);
                    segments.ending_mixed = true;
                }
                "recap" => {
                    segments.recap_start_ms = Some(start_ms);
                    segments.recap_end_ms = Some(end_ms);
                }
                _ => {}
            }
        }
    }
    Ok(segments)
}
