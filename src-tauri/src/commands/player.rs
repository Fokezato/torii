use crate::db::skip_segments::SkipSegments;
use crate::error::AppError;
use crate::player::media_tools::{MediaProbe, MediaTools};
use crate::player::{NowPlaying, PlayerEngine, PlayerSnapshot, PlayerState, TrackInfo};
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State};

const OVERLAY_LABEL: &str = "player-overlay";

fn with_engine<T>(state: &State<'_, PlayerState>, f: impl FnOnce(&PlayerEngine) -> T) -> Result<T, AppError> {
    let guard = state.engine.lock().unwrap();
    let engine = guard
        .as_ref()
        .ok_or_else(|| AppError::Fetch("player não inicializado (libvlc não carregou)".to_string()))?;
    Ok(f(engine))
}

/// `source`: path local do Windows (episódio já baixado) ou URL http(s)
/// (ex. servidor de streaming do librqbit, pra assistir sem esperar
/// terminar de baixar — ver `PlayerEngine::open`). `title`/`episode_label`/
/// `watch_id` só alimentam a barra de cima + painel de episódios do
/// overlay (ver `NowPlaying`), não afetam o libvlc.
#[tauri::command]
pub fn player_open(
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
        .ok_or_else(|| AppError::Fetch("player não inicializado (libvlc não carregou)".to_string()))?;
    engine.open(&source, start_ms).map_err(AppError::Fetch)?;
    engine.play();
    drop(guard);
    #[cfg(windows)]
    state.with_media_session(|s| s.set_active(Some((&title, &episode_label))));
    let mut now_playing = state.now_playing.lock().unwrap();
    let session = now_playing.session + 1;
    *now_playing = NowPlaying { title, episode_label, watch_id, episode_number, source, session };
    Ok(())
}

#[tauri::command]
pub fn player_play(state: State<'_, PlayerState>) -> Result<(), AppError> {
    with_engine(&state, |e| e.play())
}

#[tauri::command]
pub fn player_set_paused(state: State<'_, PlayerState>, paused: bool) -> Result<(), AppError> {
    with_engine(&state, |e| e.set_paused(paused))?;
    #[cfg(windows)]
    state.with_media_session(|s| s.set_playing(!paused));
    Ok(())
}

#[tauri::command]
pub fn player_stop(state: State<'_, PlayerState>) -> Result<(), AppError> {
    with_engine(&state, |e| e.stop())?;
    // Parou = teclas de mídia voltam pros outros apps.
    #[cfg(windows)]
    state.with_media_session(|s| s.set_active(None));
    Ok(())
}

#[tauri::command]
pub fn player_seek(state: State<'_, PlayerState>, position_ms: i64) -> Result<(), AppError> {
    with_engine(&state, |e| e.seek_ms(position_ms))
}

/// `delta_ms` negativo = voltar (botão -10s), positivo = avançar (+10s).
/// Clampa em [0, duration] — sem isso, voltar perto do início ou avançar
/// perto do fim mandava `set_time` pra fora do range e o libvlc ignorava
/// silenciosamente (botão parecia travado nas pontas).
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
pub fn player_snapshot(state: State<'_, PlayerState>) -> Result<PlayerStatus, AppError> {
    let playback = with_engine(&state, |e| e.snapshot())?;
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
        .ok_or_else(|| AppError::Fetch("leitor de mídia não inicializado".to_string()))
}

/// Duração, resolução e faixas de áudio/legenda de um arquivo (painel de
/// episódios) — lê só o cabeçalho, sem tocar. Cacheado por caminho.
#[tauri::command]
pub async fn media_probe(state: State<'_, PlayerState>, path: String) -> Result<MediaProbe, AppError> {
    let tools = media_tools(&state)?;
    tauri::async_runtime::spawn_blocking(move || tools.probe_cached(&path))
        .await
        .map_err(|e| AppError::Fetch(e.to_string()))?
        .map_err(AppError::Fetch)
}

/// Quadro do arquivo em `time_ms` como data URL JPEG — miniatura do
/// episódio e da barra do tempo. Cacheado por arquivo+tempo.
#[tauri::command]
pub async fn media_frame(state: State<'_, PlayerState>, path: String, time_ms: i64) -> Result<String, AppError> {
    let tools = media_tools(&state)?;
    tauri::async_runtime::spawn_blocking(move || tools.frame_cached(&path, time_ms))
        .await
        .map_err(|e| AppError::Fetch(e.to_string()))?
        .map_err(AppError::Fetch)
}

/// Salva onde o player está no episódio (chamado a cada ~10s pela overlay)
/// e marca "assistido" quando `watched` (chegou no encerramento/90%) — base
/// do "apagar depois de assistir" (ver `engine::cleanup_once`).
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

/// Busca os trechos de abertura/encerramento do episódio (AniSkip, ver
/// `sources::aniskip`) pro botão/auto-skip do player — cacheados em
/// `skip_segments` (por watch+episódio, não por release baixada) e o
/// `mal_id` do watch resolvido e salvo na 1ª vez (ver
/// `db::watches::set_mal_id`), pra não repetir chamada de rede depois.
#[tauri::command]
pub async fn player_get_skip_segments(
    app_state: State<'_, AppState>,
    watch_id: i64,
    episode_number: i64,
) -> Result<SkipSegments, AppError> {
    if let Some(cached) = crate::db::skip_segments::get_cached(&app_state.db, watch_id, episode_number).await? {
        return Ok(cached);
    }

    // Só grava no cache resposta DEFINITIVA ("não tem dado" de verdade ou os
    // tempos). Erro de rede devolve vazio sem gravar — senão uma falha
    // passageira marcava o episódio como "sem dados" pra sempre.
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
            Err(_) => return Ok(SkipSegments::default()),
        },
        (None, None) => return Ok(SkipSegments::default()),
    };

    match crate::sources::aniskip::fetch_skip_times(&app_state.http, mal_id, episode_number).await {
        Ok(segments) => {
            crate::db::skip_segments::upsert(&app_state.db, watch_id, episode_number, &segments).await?;
            Ok(segments)
        }
        Err(_) => Ok(SkipSegments::default()),
    }
}

/// Chamado pelo React a cada resize/scroll da área reservada pro vídeo
/// (`ResizeObserver` no front) — reposiciona a child HWND nativa por cima
/// dessa área. Coordenadas em pixels físicos da tela (já convertidas no
/// front via `devicePixelRatio` + posição da janela).
#[tauri::command]
pub fn player_resize(state: State<'_, PlayerState>, x: i32, y: i32, width: i32, height: i32) -> Result<(), AppError> {
    with_engine(&state, |e| {
        crate::player::window::resize(e.hwnd(), x, y, width, height);
    })
}

/// Reafirma o z-order do vídeo sem reposicionar — ver
/// `player::window::bring_to_front`. Chamado periodicamente pelo overlay
/// (poll de snapshot já roda a cada ~400ms, barato piggybackar nele) porque
/// o WebView2 pode reafirmar o PRÓPRIO z-order em momentos que um resize
/// único não cobre (ex. overlay terminando de inicializar).
#[tauri::command]
pub fn player_bring_to_front(app: AppHandle, state: State<'_, PlayerState>) -> Result<(), AppError> {
    with_engine(&state, |e| {
        crate::player::window::bring_to_front(e.hwnd());
    })?;

    // Overlay logo acima da principal (ver `window::ensure_above`) — mesmo
    // tick periódico, pra corrigir sozinho se a principal (ex. maximizada)
    // passar por cima dela.
    #[cfg(windows)]
    if let (Some(main), Some(overlay)) = (app.get_webview_window("main"), app.get_webview_window(OVERLAY_LABEL)) {
        if let (Ok(main_hwnd), Ok(overlay_hwnd)) = (main.hwnd(), overlay.hwnd()) {
            crate::player::window::ensure_above(overlay_hwnd, main_hwnd);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn player_set_visible(state: State<'_, PlayerState>, visible: bool) -> Result<(), AppError> {
    with_engine(&state, |e| {
        crate::player::window::set_visible(e.hwnd(), visible);
    })
}

/// Posiciona a HWND nativa do vídeo E a janela de overlay dos controles
/// (criada 1x no boot — ver `spawn_player_overlay_window` — nunca aqui)
/// numa chamada só — andam sempre juntas. `x/y/width/height` são
/// relativos à área de conteúdo da janela principal (mesmo referencial da
/// HWND filha do vídeo). A posição absoluta de tela da overlay sai desse
/// mesmo x/y via `ClientToScreen` (Win32, ver
/// `player::window::client_to_screen`), numa leitura só — sem depender de
/// 2 leituras separadas da posição da janela que driftavam durante
/// resize/move. `overlay_x/overlay_y` (calculados no front) ficam só como
/// fallback se a HWND da janela principal não puder ser obtida.
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
) -> Result<(), AppError> {
    // Minimizar a janela principal dispara "resize" no front (viewport vai
    // pra 0x0), que chama isso aqui — sem essa guarda, reposicionava a
    // overlay de volta pra tela por cima do fix que a manda pra fora
    // (ver o handler de `WindowEvent::Resized` no lib.rs), race entre os
    // dois: overlay "não sumia" ao minimizar (bug real reportado).
    // Mesma lógica pra janela escondida na bandeja (fechar com "minimizar
    // pra bandeja"): o timer de 1s da página continua rodando com a janela
    // oculta e recolocaria a overlay na tela.
    let main = app.get_webview_window("main");
    if let Some(main) = &main {
        if main.is_minimized().unwrap_or(false) || !main.is_visible().unwrap_or(true) {
            return Ok(());
        }
    }

    // Visibilidade derivada da área, na MESMA chamada: a página esconde a
    // área (-2000, 1x1) ao sair e mostra ao entrar — com show/hide em
    // chamadas separadas, o StrictMode do React (monta 2x em dev) fazia o
    // "esconder" da desmontagem chegar por último às vezes, deixando o
    // vídeo oculto com a página aberta (tela preta — visto no log real).
    // Como essa função roda a cada 1s na página, corrige sozinho.
    let on_screen = width > 1 && height > 1 && x > -1000 && y > -1000;
    with_engine(&state, |e| {
        crate::player::window::resize(e.hwnd(), x, y, width, height);
        crate::player::window::set_visible(e.hwnd(), on_screen);
    })?;

    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        #[cfg(windows)]
        let (final_x, final_y) = match main.as_ref().and_then(|m| m.hwnd().ok()) {
            Some(main_hwnd) => crate::player::window::client_to_screen(main_hwnd, x, y),
            None => (overlay_x, overlay_y),
        };
        #[cfg(not(windows))]
        let (final_x, final_y) = (overlay_x, overlay_y);
        let _ = overlay.set_position(PhysicalPosition::new(final_x, final_y));
        let _ = overlay.set_size(PhysicalSize::new(width.max(1) as u32, height.max(1) as u32));
    }
    Ok(())
}
