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
        .ok_or_else(|| AppError::Fetch(tr!("player não inicializado (libvlc não carregou)", "player not initialized (libvlc failed to load)")))?;
    Ok(f(engine))
}

/// `source`: path local do Windows (episódio já baixado) ou URL http(s)
/// (ex. servidor de streaming do librqbit, pra assistir sem esperar
/// terminar de baixar — ver `PlayerEngine::open`). `title`/`episode_label`/
/// `watch_id` só alimentam a barra de cima + painel de episódios do
/// overlay (ver `NowPlaying`), não afetam o libvlc.
/// Streaming: apaga os episódios já assistidos (menos o que está abrindo).
/// Em segundo plano — `player_open`/`player_stop` rodam na thread principal.
fn spawn_stream_cleanup(app: &AppHandle, playing: Option<String>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        crate::engine::cleanup_watched_streams(&state, playing.as_deref()).await;
    });
}

/// Discord: episódio novo. Capa e link da AniList vêm do banco (em segundo
/// plano — `player_open` roda na thread principal).
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
pub fn player_play(state: State<'_, PlayerState>) -> Result<(), AppError> {
    with_engine(&state, |e| e.play())
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
    // Parou = teclas de mídia voltam pros outros apps.
    #[cfg(any(windows, target_os = "linux"))]
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

/// Falas da faixa de legenda `ordinal` (0 = primeira faixa de legenda do
/// arquivo) pra legenda personalizada — ver `subtitles`. Baixa o ffmpeg na
/// primeira vez, se preciso.
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

/// Largura da amostra de cores: pequena de propósito — o brilho é todo
/// borrado.
const AMBIENT_FRAME_WIDTH: u32 = 64;

/// Cores atuais do vídeo pra luz ambiente, em bytes: 4 × u32 LE (largura e
/// altura da amostra, largura e altura reais do vídeo) + pixels RGBA. Lê a
/// tela na área do vídeo (ver `window::capture_colors`) e o tamanho guardado
/// em `PlayerState` — não toca no libvlc nem no `engine` (ver
/// `PlayerState::video_size`). Vazio = sem vídeo.
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
    // Tamanho do vídeo lido aqui (thread principal) e guardado pra luz
    // ambiente — ver `PlayerState::video_size`.
    let (playback, size) = with_engine(&state, |e| (e.snapshot(), e.video_size()))?;
    let packed = size.map(|(w, h)| ((w as u64) << 32) | h as u64).unwrap_or(0);
    state.video_size.store(packed, std::sync::atomic::Ordering::Relaxed);
    if let Some(presence) = app.try_state::<crate::discord::Presence>() {
        use crate::player::ffi::VlcState;
        match playback.state {
            VlcState::Stopped | VlcState::Ended | VlcState::Error => presence.clear(),
            // Abrindo/carregando conta como tocando (senão pisca "Pausado" no início).
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
    app: AppHandle,
    app_state: State<'_, AppState>,
    watch_id: i64,
    episode_number: i64,
) -> Result<SkipSegments, AppError> {
    let mut segments = aniskip_segments(&app_state, watch_id, episode_number).await?;
    if !segments.is_complete() {
        match crate::db::skip_segments::get_detected(&app_state.db, watch_id, episode_number).await? {
            Some(detected) => segments.fill_from(&detected),
            // Ainda não analisado: roda a detecção em segundo plano (vale
            // pro próximo episódio / próxima vez que abrir).
            None => crate::intro_detect::spawn_pending(&app),
        }
    }
    Ok(segments)
}

/// Trechos do AniSkip, com cache no banco.
async fn aniskip_segments(
    app_state: &AppState,
    watch_id: i64,
    episode_number: i64,
) -> Result<SkipSegments, AppError> {
    // O AniSkip é colaborativo: trecho que falta hoje pode ser marcado por
    // alguém depois. Resultado incompleto vale 1 dia; completo, pra sempre.
    let cached = crate::db::skip_segments::get_cached(&app_state.db, watch_id, episode_number).await?;
    if let Some((segments, fetched_at)) = &cached {
        if segments.is_complete() || chrono::Utc::now() - *fetched_at < chrono::Duration::days(1) {
            return Ok(segments.clone());
        }
    }
    let fallback = cached.map(|(segments, _)| segments).unwrap_or_default();

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

/// Chamado pelo React a cada resize/scroll da área reservada pro vídeo
/// (`ResizeObserver` no front) — reposiciona a child HWND nativa por cima
/// dessa área. Coordenadas em pixels físicos da tela (já convertidas no
/// front via `devicePixelRatio` + posição da janela).
#[tauri::command]
pub fn player_resize(state: State<'_, PlayerState>, x: i32, y: i32, width: i32, height: i32) -> Result<(), AppError> {
    with_engine(&state, |e| {
        crate::player::window::resize(e.surface(), x, y, width, height);
    })
}

/// Reafirma o z-order do vídeo sem reposicionar — ver
/// `player::window::bring_to_front`. Chamado periodicamente pelo overlay
/// (poll de snapshot já roda a cada ~400ms, barato piggybackar nele) porque
/// o WebView2 pode reafirmar o PRÓPRIO z-order em momentos que um resize
/// único não cobre (ex. overlay terminando de inicializar).
#[tauri::command]
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn player_bring_to_front(app: AppHandle, state: State<'_, PlayerState>) -> Result<(), AppError> {
    with_engine(&state, |e| {
        crate::player::window::bring_to_front(e.surface());
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

/// Tira a overlay dos controles da tela. Windows: manda pra fora da tela
/// (esconder/mostrar a janela bagunçava a relação de owner). Linux: o
/// gerenciador de janelas traz de volta janela fora da tela, então esconde
/// de verdade.
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
    video: Option<[i32; 4]>,
) -> Result<(), AppError> {
    // Minimizar a janela principal dispara "resize" no front (viewport vai
    // pra 0x0), que chama isso aqui — sem essa guarda, reposicionava a
    // overlay de volta pra tela por cima do fix que a manda pra fora
    // (ver o handler de `WindowEvent::Resized` no lib.rs), race entre os
    // dois: overlay "não sumia" ao minimizar (bug real reportado).
    // Mesma lógica pra janela escondida na bandeja (fechar com "minimizar
    // pra bandeja"): o timer de 1s da página continua rodando com a janela
    // oculta e recolocaria a overlay na tela.
    // Visibilidade derivada da área, na MESMA chamada: a página esconde a
    // área (-2000, 1x1) ao sair e mostra ao entrar — com show/hide em
    // chamadas separadas, o StrictMode do React (monta 2x em dev) fazia o
    // "esconder" da desmontagem chegar por último às vezes, deixando o
    // vídeo oculto com a página aberta (tela preta — visto no log real).
    // Como essa função roda a cada 1s na página, corrige sozinho.
    let on_screen = width > 1 && height > 1 && x > -1000 && y > -1000;

    // A guarda só barra MOSTRAR: esconder tem que passar sempre. Fechar pra
    // bandeja com o player aberto sai da página com a janela já oculta — se
    // o "esconder" fosse ignorado, a HWND do vídeo ficava parada por cima da
    // interface e, ao reabrir, nada recebia clique (bug real).
    let main = app.get_webview_window("main");
    if let Some(main) = &main {
        if on_screen && (main.is_minimized().unwrap_or(false) || !main.is_visible().unwrap_or(true)) {
            return Ok(());
        }
    }
    // `video`: retângulo só da imagem (luz ambiente ligada — o espaço em
    // volta vira área da página, onde o brilho é desenhado). Sem ele, o
    // vídeo ocupa a área toda e o VLC pinta as faixas pretas por dentro.
    // Os controles (overlay) cobrem sempre a área toda.
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
