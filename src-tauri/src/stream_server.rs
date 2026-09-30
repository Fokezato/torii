use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::hash::{BuildHasher, Hasher};
use std::io::SeekFrom;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[derive(Clone)]
pub struct StreamServer {
    port: u16,
    token: String,
}

impl StreamServer {
    pub fn url(&self, episode_id: i64, extension: &str) -> String {
        format!("http://127.0.0.1:{}/stream/{}/{episode_id}.{extension}", self.port, self.token)
    }
}

pub fn episode_of_url(url: &str) -> Option<i64> {
    if !url.starts_with("http://127.0.0.1:") || !url.contains("/stream/") {
        return None;
    }
    url.rsplit('/').next()?.split('.').next()?.parse().ok()
}

fn random_token() -> String {
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

pub async fn start(app: AppHandle) -> anyhow::Result<StreamServer> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let server = StreamServer { port: listener.local_addr()?.port(), token: random_token() };
    let router = Router::new()
        .route("/stream/{token}/{file}", get(serve))
        .with_state((app, server.token.clone()));
    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("[stream] servidor parou: {e}");
        }
    });
    Ok(server)
}

fn parse_range(value: &str, len: u64) -> Option<(u64, u64)> {
    let spec = value.strip_prefix("bytes=")?.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            (len.saturating_sub(n), len - 1)
        }
        (s, "") => (s.parse().ok()?, len - 1),
        (s, e) => (s.parse().ok()?, e.parse::<u64>().ok()?.min(len - 1)),
    };
    (start <= end && start < len).then_some((start, end))
}

async fn serve(
    State((app, token)): State<(AppHandle, String)>,
    Path((given, file)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    if given != token {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(episode_id) = file.split('.').next().and_then(|s| s.parse::<i64>().ok()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let state = app.state::<AppState>();
    let Some((handle, file_id, len, name)) = state.torrent.stream_target(episode_id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if len == 0 {
        return StatusCode::NOT_FOUND.into_response();
    }
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| parse_range(v, len));
    let (start, end) = range.unwrap_or((0, len - 1));

    let mut stream = match handle.stream(file_id).await {
        Ok(s) => s,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    if stream.seek(SeekFrom::Start(start)).await.is_err() {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    }
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(stream.take(end - start + 1)));

    let content_type = if name.to_ascii_lowercase().ends_with(".mp4") { "video/mp4" } else { "video/x-matroska" };
    let mut response = Response::new(body);
    *response.status_mut() = if range.is_some() { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK };
    let h = response.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(end - start + 1));
    if range.is_some() {
        if let Ok(v) = HeaderValue::from_str(&format!("bytes {start}-{end}/{len}")) {
            h.insert(header::CONTENT_RANGE, v);
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ranges() {
        assert_eq!(parse_range("bytes=0-", 1000), Some((0, 999)));
        assert_eq!(parse_range("bytes=100-199", 1000), Some((100, 199)));
        assert_eq!(parse_range("bytes=900-5000", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=2000-", 1000), None);
    }

    #[test]
    fn finds_episode_in_stream_url() {
        let s = StreamServer { port: 1234, token: "abc".into() };
        assert_eq!(episode_of_url(&s.url(42, "mkv")), Some(42));
        assert_eq!(episode_of_url("C:\\Videos\\ep.mkv"), None);
    }
}
