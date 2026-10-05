use crate::db::skip_segments::SkipSegments;
use crate::error::AppError;
use crate::player::media_tools::{MediaProbe, MediaTools};
use crate::player::{NowPlaying, PlayerEngine, PlayerSnapshot, PlayerState, TrackInfo};
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State};

const OVERLAY_LABEL: &str = "player-overlay";

fn with_engine<T>(state: &State<'_, PlayerState>, f: impl FnOnce(&PlayerEngine) -> T) -> Result<T, AppError> {
    let guard = state.engine.lock().unwrap();
    let engine = guard
        .as_ref()
        .ok_or_else(|| AppError::Fetch(tr!("player não inicializado (libvlc não carregou)", "player not initialized (libvlc failed to load)")))?;
    Ok(f(engine))
}

fn spawn_stream_cleanup(app: &AppHandle, playing: Option<String>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        crate::engine::cleanup_watched_streams(&state, playing.as_deref()).await;
    });
}

fn spawn_discord_open(app: &AppHandle, title: String, episode: String, watch_id: Option<i64>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(presence) = app.try_state::<crate::discord::Presence>() else { return };
        let state = app.state::<AppState>();
        let enabled = crate::db::settings::get_all(&state.db)
            .await
            .map(|s| s.get("discord_presence").map(String::as_str) != Some("0"))
            .unwrap_or(true);
        presence.set_enabled(enabled);
        let watch = match watch_id {
            Some(id) => crate::db::watches::get(&state.db, id).await.ok(),
            None => None,
        };
        presence.open(crate::discord::Meta {
            title,
            episode,
            cover_url: watch.as_ref().and_then(|w| w.cover_url.clone()),
            anilist_id: watch.as_ref().and_then(|w| w.anilist_id),
        });
    });
}

#[tauri::command]
pub fn player_open(
    app: AppHandle,
    state: State<'_, PlayerState>,
    source: String,
    title: String,
    episode_label: String,
    watch_id: Option<i64>,
    episode_number: Option<i64>,
    start_ms: Option<i64>,
) -> Result<(), AppError> {
    let mut guard = state.engine.lock().unwrap();
    let engine = guard
        .as_mut()
        .ok_or_else(|| AppError::Fetch(tr!("player não inicializado (libvlc não carregou)", "player not initialized (libvlc failed to load)")))?;
    engine.open(&source, start_ms).map_err(AppError::Fetch)?;
    engine.play();
    drop(guard);
    #[cfg(any(windows, target_os = "linux"))]
    state.with_media_session(|s| s.set_active(Some((&title, &episode_label))));
    spawn_stream_cleanup(&app, Some(source.clone()));
    spawn_discord_open(&app, title.clone(), episode_label.clone(), watch_id);
    let mut now_playing = state.now_playing.lock().unwrap();
    let session = now_playing.session + 1;
    *now_playing = NowPlaying { title, episode_label, watch_id, episode_number, source, session };
    Ok(())
}

#[tauri::command]
pub fn player_set_paused(state: State<'_, PlayerState>, paused: bool) -> Result<(), AppError> {
    with_engine(&state, |e| e.set_paused(paused))?;
    #[cfg(any(windows, target_os = "linux"))]
    state.with_media_session(|s| s.set_playing(!paused));
    Ok(())
}

#[tauri::command]
pub fn player_stop(app: AppHandle, state: State<'_, PlayerState>) -> Result<(), AppError> {
    with_engine(&state, |e| e.stop())?;
    spawn_stream_cleanup(&app, None);
    if let Some(presence) = app.try_state::<crate::discord::Presence>() {
        presence.clear();
    }
    #[cfg(any(windows, target_os = "linux"))]
    state.with_media_session(|s| s.set_active(None));
    Ok(())
}

#[tauri::command]
pub fn player_seek(state: State<'_, PlayerState>, position_ms: i64) -> Result<(), AppError> {
    with_engine(&state, |e| e.seek_ms(position_ms))
}

#[tauri::command]
pub fn player_seek_relative(state: State<'_, PlayerState>, delta_ms: i64) -> Result<(), AppError> {
    with_engine(&state, |e| {
        let snap = e.snapshot();
        let target = (snap.position_ms + delta_ms).clamp(0, snap.duration_ms.max(0));
        e.seek_ms(target);
    })
}

#[tauri::command]
pub fn player_set_volume(state: State<'_, PlayerState>, volume: i32) -> Result<(), AppError> {
    with_engine(&state, |e| e.set_volume(volume))
}

#[tauri::command]
pub fn player_list_audio_tracks(state: State<'_, PlayerState>) -> Result<Vec<TrackInfo>, AppError> {
    with_engine(&state, |e| e.list_audio_tracks())
}

#[tauri::command]
pub fn player_set_audio_track(state: State<'_, PlayerState>, id: i32) -> Result<(), AppError> {
    with_engine(&state, |e| e.set_audio_track(id))
}

#[tauri::command]
pub fn player_list_subtitle_tracks(state: State<'_, PlayerState>) -> Result<Vec<TrackInfo>, AppError> {
    with_engine(&state, |e| e.list_subtitle_tracks())
}

#[tauri::command]
pub fn player_set_subtitle_track(state: State<'_, PlayerState>, id: i32) -> Result<(), AppError> {
    with_engine(&state, |e| e.set_subtitle_track(id))
}

#[derive(Serialize)]
pub struct PlayerStatus {
    #[serde(flatten)]
    pub playback: PlayerSnapshot,
    pub title: String,
    pub episode_label: String,
    pub watch_id: Option<i64>,
    pub episode_number: Option<i64>,
    pub source: String,
    pub session: u64,
}

#[tauri::command]
pub async fn player_subtitle_cues(
    app: AppHandle,
    app_state: State<'_, AppState>,
    path: String,
    ordinal: u32,
) -> Result<Vec<crate::subtitles::Cue>, AppError> {
    let paths = crate::ffmpeg::ensure_installed(&app, &app_state.http).await.map_err(AppError::Fetch)?;
    tauri::async_runtime::spawn_blocking(move || crate::subtitles::cues(&paths, std::path::Path::new(&path), ordinal))
        .await
        .map_err(|e| AppError::Fetch(e.to_string()))?
        .map(|cues| cues.as_ref().clone())
        .map_err(AppError::Fetch)
}

const AMBIENT_FRAME_WIDTH: u32 = 64;

#[tauri::command]
pub async fn player_ambient_frame(app: AppHandle) -> tauri::ipc::Response {
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        use std::sync::atomic::Ordering;
        let state = app.state::<PlayerState>();
        let packed = state.video_size.load(Ordering::Relaxed);
        let (vw, vh) = ((packed >> 32) as u32, packed as u32);
        let raw_hwnd = state.video_hwnd.load(Ordering::Relaxed);
        if vw == 0 || vh == 0 || raw_hwnd == 0 {
            return Vec::new();
        }
        let capture = crate::player::window::capture_colors(
            crate::player::window::from_raw(raw_hwnd),
            AMBIENT_FRAME_WIDTH,
        );
        let Some((w, h, pixels)) = capture else { return Vec::new() };
        let mut out = Vec::with_capacity(16 + pixels.len());
        for n in [w, h, vw, vh] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(&pixels);
        out
    })
    .await
    .unwrap_or_default();
    tauri::ipc::Response::new(bytes)
}

#[tauri::command]
pub fn player_snapshot(app: AppHandle, state: State<'_, PlayerState>) -> Result<PlayerStatus, AppError> {
    let (playback, size) = with_engine(&state, |e| (e.snapshot(), e.video_size()))?;
    let packed = size.map(|(w, h)| ((w as u64) << 32) | h as u64).unwrap_or(0);
    state.video_size.store(packed, std::sync::atomic::Ordering::Relaxed);
    if let Some(presence) = app.try_state::<crate::discord::Presence>() {
        use crate::player::ffi::VlcState;
        match playback.state {
            VlcState::Stopped | VlcState::Ended | VlcState::Error => presence.clear(),
            state => presence.playback(state != VlcState::Paused, playback.position_ms, playback.duration_ms),
        }
    }
    let now_playing = state.now_playing.lock().unwrap().clone();
    Ok(PlayerStatus {
        playback,
        title: now_playing.title,
        episode_label: now_playing.episode_label,
        watch_id: now_playing.watch_id,
        episode_number: now_playing.episode_number,
        source: now_playing.source,
        session: now_playing.session,
    })
}

fn media_tools(state: &State<'_, PlayerState>) -> Result<MediaTools, AppError> {
    with_engine(state, |e| e.tools())?
        .ok_or_else(|| AppError::Fetch(tr!("leitor de mídia não inicializado", "media reader not initialized")))
}

#[tauri::command]
pub async fn media_probe(state: State<'_, PlayerState>, path: String) -> Result<MediaProbe, AppError> {
    let tools = media_tools(&state)?;
    tauri::async_runtime::spawn_blocking(move || tools.probe_cached(&path))
        .await
        .map_err(|e| AppError::Fetch(e.to_string()))?
        .map_err(AppError::Fetch)
}

#[tauri::command]
pub async fn media_frame(state: State<'_, PlayerState>, path: String, time_ms: i64) -> Result<String, AppError> {
    let tools = media_tools(&state)?;
    tauri::async_runtime::spawn_blocking(move || tools.frame_cached(&path, time_ms))
        .await
        .map_err(|e| AppError::Fetch(e.to_string()))?
        .map_err(AppError::Fetch)
}

#[tauri::command]
pub async fn player_save_progress(
    app_state: State<'_, AppState>,
    watch_id: i64,
    episode_number: i64,
    position_ms: i64,
    watched: bool,
) -> Result<(), AppError> {
    crate::db::episodes::save_progress(&app_state.db, watch_id, episode_number, position_ms.max(0), watched).await?;
    Ok(())
}

#[tauri::command]
pub async fn player_get_skip_segments(
    app: AppHandle,
    app_state: State<'_, AppState>,
    watch_id: i64,
    episode_number: i64,
) -> Result<SkipSegments, AppError> {
    // Chapters are marked in this exact file, so they win over AniSkip's shared timings.
    let mut segments = file_chapter_segments(&app, &app_state, watch_id, episode_number).await;
    if !segments.is_complete() {
        // Answer with what is cached; AniSkip can take many seconds, so it is fetched in
        // the background and the player is told to ask again.
        let cached = crate::db::skip_segments::get_cached(&app_state.db, watch_id, episode_number).await?;
        let fresh = cached
            .as_ref()
            .is_some_and(|(s, at)| s.is_complete() || chrono::Utc::now() - *at < chrono::Duration::days(1));
        if let Some((aniskip, _)) = &cached {
            segments.fill_from(aniskip);
        }
        if !fresh {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                if aniskip_segments(&state, watch_id, episode_number).await.is_ok() {
                    let _ = app.emit(
                        "player:skip-segments-updated",
                        serde_json::json!({ "watch_id": watch_id, "episode_number": episode_number }),
                    );
                }
            });
        }
    }
    if !segments.is_complete() {
        match crate::db::skip_segments::get_detected(&app_state.db, watch_id, episode_number).await? {
            Some(detected) => segments.fill_from(&detected),
            None => crate::intro_detect::spawn_pending(&app),
        }
    }
    Ok(segments)
}

async fn file_chapter_segments(app: &AppHandle, app_state: &AppState, watch_id: i64, episode_number: i64) -> SkipSegments {
    let Ok(episodes) = crate::db::episodes::list_for_watch(&app_state.db, watch_id).await else {
        return SkipSegments::default();
    };
    let path = episodes
        .into_iter()
        .find(|e| e.episode_number == Some(episode_number) && e.status == "available")
        .and_then(|e| e.item_path);
    match path {
        Some(path) => crate::intro_detect::file_chapters(app, &path).await,
        None => SkipSegments::default(),
    }
}

async fn aniskip_segments(
    app_state: &AppState,
    watch_id: i64,
    episode_number: i64,
) -> Result<SkipSegments, AppError> {
    let cached = crate::db::skip_segments::get_cached(&app_state.db, watch_id, episode_number).await?;
    if let Some((segments, fetched_at)) = &cached {
        if segments.is_complete() || chrono::Utc::now() - *fetched_at < chrono::Duration::days(1) {
            return Ok(segments.clone());
        }
    }
    let fallback = cached.map(|(segments, _)| segments).unwrap_or_default();

    let watch = crate::db::watches::get(&app_state.db, watch_id).await?;
    let mal_id = match (watch.mal_id, watch.anilist_id) {
        (Some(id), _) => id,
        (None, Some(anilist_id)) => match crate::sources::aniskip::fetch_mal_id(&app_state.http, anilist_id).await {
            Ok(Some(id)) => {
                crate::db::watches::set_mal_id(&app_state.db, watch_id, id).await?;
                id
            }
            Ok(None) => {
                crate::db::skip_segments::upsert(&app_state.db, watch_id, episode_number, &SkipSegments::default())
                    .await?;
                return Ok(SkipSegments::default());
            }
            Err(_) => return Ok(fallback),
        },
        (None, None) => return Ok(fallback),
    };

    match crate::sources::aniskip::fetch_skip_times(&app_state.http, mal_id, episode_number).await {
        Ok(segments) => {
            crate::db::skip_segments::upsert(&app_state.db, watch_id, episode_number, &segments).await?;
            Ok(segments)
        }
        Err(_) => Ok(fallback),
    }
}

#[tauri::command]
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn player_bring_to_front(app: AppHandle, state: State<'_, PlayerState>) -> Result<(), AppError> {
    with_engine(&state, |e| {
        crate::player::window::bring_to_front(e.surface());
    })?;

    #[cfg(windows)]
    if let (Some(main), Some(overlay)) = (app.get_webview_window("main"), app.get_webview_window(OVERLAY_LABEL)) {
        if let (Ok(main_hwnd), Ok(overlay_hwnd)) = (main.hwnd(), overlay.hwnd()) {
            crate::player::window::ensure_above(overlay_hwnd, main_hwnd);
        }
    }
    Ok(())
}

pub fn hide_overlay(overlay: &tauri::WebviewWindow) {
    #[cfg(windows)]
    let _ = overlay.set_position(PhysicalPosition::new(-32000, -32000));
    #[cfg(not(windows))]
    let _ = overlay.hide();
}

#[tauri::command]
pub fn player_set_visible(state: State<'_, PlayerState>, visible: bool) -> Result<(), AppError> {
    with_engine(&state, |e| {
        crate::player::window::set_visible(e.surface(), visible);
    })
}

#[tauri::command]
pub fn player_set_video_area(
    app: AppHandle,
    state: State<'_, PlayerState>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    overlay_x: i32,
    overlay_y: i32,
    video: Option<[i32; 4]>,
) -> Result<(), AppError> {
    let on_screen = width > 1 && height > 1 && x > -1000 && y > -1000;

    let main = app.get_webview_window("main");
    if let Some(main) = &main {
        if on_screen && (main.is_minimized().unwrap_or(false) || !main.is_visible().unwrap_or(true)) {
            return Ok(());
        }
    }
    let [vx, vy, vw, vh] = video.unwrap_or([x, y, width, height]);
    with_engine(&state, |e| {
        crate::player::window::resize(e.surface(), vx, vy, vw, vh);
        crate::player::window::set_visible(e.surface(), on_screen);
    })?;

    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let (final_x, final_y) = match main.as_ref().and_then(crate::player::window::main_surface) {
            Some(main_surface) => crate::player::window::client_to_screen(main_surface, x, y),
            None => (overlay_x, overlay_y),
        };
        #[cfg(not(windows))]
        if !on_screen {
            hide_overlay(&overlay);
            return Ok(());
        }
        let _ = overlay.set_position(PhysicalPosition::new(final_x, final_y));
        let _ = overlay.set_size(PhysicalSize::new(width.max(1) as u32, height.max(1) as u32));
        #[cfg(not(windows))]
        if !overlay.is_visible().unwrap_or(true) {
            let _ = overlay.show();
        }
    }
    Ok(())
}
