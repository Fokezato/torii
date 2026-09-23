pub mod ffi;
#[cfg(windows)]
pub mod media_session;
pub mod media_tools;
pub mod window;

use ffi::{LibvlcInstance, LibvlcMedia, LibvlcMediaPlayer, LibvlcTrackDescription, VlcApi, VlcState};
use libloading::Library;
use std::ffi::{CStr, CString};
use windows::Win32::Foundation::HWND;

/// Motor de playback: carrega libvlc.dll, mantém instância + media player
/// vivos, embutidos na HWND filha criada por `window::create_child`. Fica
/// em `AppState` atrás de `Mutex` — chamadas de controle do libvlc (play/
/// pause/seek/etc.) são thread-safe por design da própria lib, então um
/// mutex simples (sem precisar rodar tudo numa thread dedicada) já basta.
pub struct PlayerEngine {
    // Precisa ficar viva pelo tempo todo — os ponteiros de função em `api`
    // apontam pra dentro dela. Nunca lida diretamente, só seguro aqui.
    _lib: Library,
    api: VlcApi,
    instance: *mut LibvlcInstance,
    player: *mut LibvlcMediaPlayer,
    media: Option<*mut LibvlcMedia>,
    hwnd: HWND,
    /// Instância separada pra miniatura/leitura de arquivo (ver
    /// `media_tools`) — `None` se não conseguiu criar; o player segue igual.
    tools: Option<media_tools::MediaTools>,
}

// libvlc_media_player_* é seguro de chamar de qualquer thread (documentado
// pela própria VideoLAN) — o único estado "não-Send" real aqui é a Library
// (ponteiros de função), que só é lida, nunca mutada após o load.
unsafe impl Send for PlayerEngine {}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerSnapshot {
    pub state: VlcState,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub volume: i32,
    pub is_playing: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TrackInfo {
    /// -1 = "Desabilitado" (legenda off / sem áudio) — já vem assim na
    /// lista do libvlc, não é caso especial tratado à parte.
    pub id: i32,
    pub name: String,
    pub active: bool,
}

/// Anda a lista ligada devolvida por `*_get_track_description`/
/// `spu_get_description`, convertendo pra `Vec` seguro, e libera a lista
/// original antes de voltar — sem isso cada chamada vazava memória do
/// lado do libvlc.
unsafe fn collect_tracks(api: &VlcApi, head: *mut LibvlcTrackDescription, active_id: i32) -> Vec<TrackInfo> {
    let mut out = Vec::new();
    let mut node = head;
    while !node.is_null() {
        let id = (*node).i_id;
        let name = if (*node).psz_name.is_null() {
            String::new()
        } else {
            CStr::from_ptr((*node).psz_name).to_string_lossy().into_owned()
        };
        out.push(TrackInfo { id, name, active: id == active_id });
        node = (*node).p_next;
    }
    if !head.is_null() {
        (api.track_description_list_release)(head);
    }
    out
}

impl PlayerEngine {
    /// `hwnd`: a child window já criada (ver `window::create_child`) onde o
    /// vídeo vai renderizar.
    pub fn new(hwnd: HWND) -> Result<Self, String> {
        let exe_dir = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or_else(|| "sem diretório pai do executável".to_string())?
            .to_path_buf();
        let dll_path = exe_dir.join("libvlc.dll");

        // SAFETY: libvlc.dll é uma DLL confiável, vendorizada pelo próprio
        // projeto (ver src-tauri/vendor/vlc) — não é input de usuário.
        let lib = unsafe { Library::new(&dll_path) }
            .map_err(|e| format!("falha ao carregar libvlc.dll em {}: {e}", dll_path.display()))?;
        // SAFETY: acabou de carregar o libvlc.dll de verdade acima; os
        // símbolos resolvidos batem com a ABI documentada do libvlc 3.x.
        let api = unsafe { ffi::load(&lib) }?;

        let plugin_path = exe_dir.join("plugins");
        // "--plugin-path=" também funcionaria sem isso (o libvlc acha a
        // pasta "plugins" sozinho do lado do próprio .dll), mas explícito
        // elimina qualquer ambiguidade de working directory.
        let plugin_arg = CString::new(format!("--plugin-path={}", plugin_path.display()))
            .map_err(|e| e.to_string())?;
        let quiet_arg = CString::new("--quiet").map_err(|e| e.to_string())?;
        let args = [plugin_arg.as_ptr(), quiet_arg.as_ptr()];

        // SAFETY: `api.new` veio do load acima, args são CStrings válidas
        // vivas até o fim desse escopo (a chamada é síncrona).
        let instance = unsafe { (api.new)(args.len() as i32, args.as_ptr()) };
        if instance.is_null() {
            return Err("libvlc_new retornou nulo".to_string());
        }

        // SAFETY: instance não-nulo, acabou de ser criado.
        let player = unsafe { (api.player_new)(instance) };
        if player.is_null() {
            unsafe { (api.release)(instance) };
            return Err("libvlc_media_player_new retornou nulo".to_string());
        }

        // SAFETY: player não-nulo; hwnd.0 é o handle Win32 cru da child
        // window já criada por quem chama.
        unsafe { (api.player_set_hwnd)(player, hwnd.0) };

        // SAFETY: mesmos args válidos da instância principal.
        let tools_instance = unsafe { (api.new)(args.len() as i32, args.as_ptr()) };
        let tools = (!tools_instance.is_null()).then(|| media_tools::MediaTools::new(api, tools_instance));

        Ok(Self { _lib: lib, api, instance, player, media: None, hwnd, tools })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn tools(&self) -> Option<media_tools::MediaTools> {
        self.tools
    }

    /// Aceita path local do Windows OU URL http(s) (ex. servidor de
    /// streaming do librqbit) — path local usa `media_new_path` (o libvlc
    /// cuida da conversão pra file:// internamente, sem precisar escapar
    /// nada na mão).
    /// `start_ms`: continuar de onde parou — vira `:start-time` do próprio
    /// libvlc (já abre no ponto, sem piscar o começo e pular depois).
    pub fn open(&mut self, source: &str, start_ms: Option<i64>) -> Result<(), String> {
        // SAFETY: player/media atuais são válidos (ou não existem ainda).
        unsafe { (self.api.player_stop)(self.player) };
        if let Some(old) = self.media.take() {
            unsafe { (self.api.media_release)(old) };
        }

        let is_url = source.starts_with("http://") || source.starts_with("https://");
        let c_source = CString::new(source).map_err(|e| e.to_string())?;
        // SAFETY: instance válido, c_source vive até o fim da chamada.
        let media = unsafe {
            if is_url {
                (self.api.media_new_location)(self.instance, c_source.as_ptr())
            } else {
                (self.api.media_new_path)(self.instance, c_source.as_ptr())
            }
        };
        if media.is_null() {
            return Err(format!("libvlc não conseguiu abrir mídia: {source}"));
        }
        if let Some(ms) = start_ms.filter(|ms| *ms > 0) {
            let option = CString::new(format!(":start-time={:.3}", ms as f64 / 1000.0)).map_err(|e| e.to_string())?;
            // SAFETY: media válido, option vive até o fim da chamada.
            unsafe { (self.api.media_add_option)(media, option.as_ptr()) };
        }

        // SAFETY: player e media válidos e não-nulos.
        unsafe { (self.api.player_set_media)(self.player, media) };
        self.media = Some(media);
        Ok(())
    }

    pub fn play(&self) {
        // SAFETY: player sempre válido pela vida do `PlayerEngine`.
        unsafe { (self.api.player_play)(self.player) };
    }

    pub fn set_paused(&self, paused: bool) {
        unsafe { (self.api.player_set_pause)(self.player, paused as i32) };
    }

    pub fn stop(&self) {
        unsafe { (self.api.player_stop)(self.player) };
    }

    pub fn seek_ms(&self, position_ms: i64) {
        unsafe { (self.api.player_set_time)(self.player, position_ms) };
    }

    pub fn set_volume(&self, volume: i32) {
        unsafe { (self.api.audio_set_volume)(self.player, volume.clamp(0, 100)) };
    }

    pub fn list_audio_tracks(&self) -> Vec<TrackInfo> {
        unsafe {
            let active = (self.api.audio_get_track)(self.player);
            let head = (self.api.audio_get_track_description)(self.player);
            collect_tracks(&self.api, head, active)
        }
    }

    pub fn set_audio_track(&self, id: i32) {
        unsafe { (self.api.audio_set_track)(self.player, id) };
    }

    pub fn list_subtitle_tracks(&self) -> Vec<TrackInfo> {
        unsafe {
            let active = (self.api.spu_get)(self.player);
            let head = (self.api.spu_get_description)(self.player);
            collect_tracks(&self.api, head, active)
        }
    }

    pub fn set_subtitle_track(&self, id: i32) {
        unsafe { (self.api.spu_set)(self.player, id) };
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        unsafe {
            PlayerSnapshot {
                state: VlcState::from((self.api.player_get_state)(self.player)),
                position_ms: (self.api.player_get_time)(self.player).max(0),
                duration_ms: (self.api.player_get_length)(self.player).max(0),
                volume: (self.api.audio_get_volume)(self.player),
                is_playing: (self.api.player_is_playing)(self.player) != 0,
            }
        }
    }
}

impl Drop for PlayerEngine {
    fn drop(&mut self) {
        unsafe {
            if let Some(media) = self.media {
                (self.api.media_release)(media);
            }
            (self.api.player_release)(self.player);
            (self.api.release)(self.instance);
        }
        if let Some(tools) = self.tools {
            tools.release();
        }
    }
}

/// Metadado de exibição (nome do anime/episódio) pro overlay mostrar na
/// barra de cima — não é estado do libvlc, só o que `player_open` (ou
/// quem chama) informou por último. Ver `commands/player.rs::player_open`.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct NowPlaying {
    pub title: String,
    pub episode_label: String,
    /// Pra painel "Episódios" (estilo Netflix) do overlay conseguir buscar
    /// a lista real (`list_watch_episodes`) sem precisar de outro canal —
    /// PlayerOverlay é uma janela/render separado, só sabe o que passou
    /// por aqui (ver `commands/player.rs::player_open`).
    pub watch_id: Option<i64>,
    /// Número do episódio — junto com `watch_id`, chave pro cache de skip
    /// segments (ver `commands/player.rs::player_get_skip_segments`).
    pub episode_number: Option<i64>,
    /// Arquivo/URL tocando — pra miniatura da barra do tempo (ver
    /// `media_tools`) saber de onde tirar o quadro.
    pub source: String,
    /// Sobe a cada `player_open` — a overlay usa pra saber que é uma
    /// abertura NOVA mesmo sendo o mesmo episódio (sair e voltar reabre o
    /// arquivo na faixa padrão; pelo título ela achava que era a mesma
    /// sessão e não reaplicava o idioma preferido — bug real reportado).
    pub session: u64,
}

pub struct PlayerState {
    /// `None` até o `setup()` da app conseguir criar a child window +
    /// carregar libvlc (precisa da HWND da janela principal, que só existe
    /// depois que a janela é criada) — e continua `None` pra sempre se o
    /// load falhar (ex. DLL faltando), sem derrubar o resto do app.
    /// Comandos checam e devolvem erro claro em vez de panicar.
    pub engine: std::sync::Mutex<Option<PlayerEngine>>,
    pub now_playing: std::sync::Mutex<NowPlaying>,
    /// Teclas de mídia + painel de mídia do Windows (ver `media_session`).
    /// `None` se não deu pra registrar — o player funciona igual sem.
    #[cfg(windows)]
    pub media_session: std::sync::Mutex<Option<media_session::MediaSession>>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            engine: std::sync::Mutex::new(None),
            now_playing: std::sync::Mutex::new(NowPlaying::default()),
            #[cfg(windows)]
            media_session: std::sync::Mutex::new(None),
        }
    }
}

impl PlayerState {
    #[cfg(windows)]
    pub fn with_media_session(&self, f: impl FnOnce(&media_session::MediaSession)) {
        if let Some(session) = self.media_session.lock().unwrap().as_ref() {
            f(session);
        }
    }
}
