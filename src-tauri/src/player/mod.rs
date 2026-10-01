pub mod ffi;
#[cfg(windows)]
pub mod media_session;
#[cfg(target_os = "linux")]
#[path = "media_session_linux.rs"]
pub mod media_session;
pub mod media_tools;
pub mod window;

use ffi::{LibvlcInstance, LibvlcMedia, LibvlcMediaPlayer, LibvlcTrackDescription, VlcApi, VlcState};
use libloading::Library;
use std::ffi::{CStr, CString};
use window::Surface;

pub struct PlayerEngine {
    _lib: Library,
    api: VlcApi,
    instance: *mut LibvlcInstance,
    player: *mut LibvlcMediaPlayer,
    media: Option<*mut LibvlcMedia>,
    surface: Surface,
    tools: Option<media_tools::MediaTools>,
}

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
    pub id: i32,
    pub name: String,
    pub active: bool,
}

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

#[cfg(windows)]
fn load_libvlc() -> Result<(Library, Vec<CString>), String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or_else(|| "executable has no parent directory".to_string())?
        .to_path_buf();
    let dll_path = exe_dir.join("libvlc.dll");
    let lib = unsafe { Library::new(&dll_path) }
        .map_err(|e| format!("failed to load libvlc.dll from {}: {e}", dll_path.display()))?;
    let plugin_arg = CString::new(format!("--plugin-path={}", exe_dir.join("plugins").display()))
        .map_err(|e| e.to_string())?;
    Ok((lib, vec![plugin_arg, CString::new("--quiet").unwrap()]))
}

#[cfg(not(windows))]
fn load_libvlc() -> Result<(Library, Vec<CString>), String> {
    let lib = unsafe { Library::new("libvlc.so.5") }.map_err(|e| {
        tr!("libVLC não encontrado ({e}) — instale o VLC pelo gerenciador de pacotes", "libVLC not found ({e}) — install VLC with your package manager")
    })?;
    Ok((lib, vec![CString::new("--quiet").unwrap(), CString::new("--no-xlib").unwrap()]))
}

impl PlayerEngine {
    pub fn new(surface: Surface) -> Result<Self, String> {
        let (lib, arg_strings) = load_libvlc()?;
        let api = unsafe { ffi::load(&lib) }?;
        let args: Vec<*const std::os::raw::c_char> = arg_strings.iter().map(|a| a.as_ptr()).collect();

        let instance = unsafe { (api.new)(args.len() as i32, args.as_ptr()) };
        if instance.is_null() {
            return Err("libvlc_new retornou nulo".to_string());
        }

        let player = unsafe { (api.player_new)(instance) };
        if player.is_null() {
            unsafe { (api.release)(instance) };
            return Err("libvlc_media_player_new retornou nulo".to_string());
        }

        #[cfg(windows)]
        unsafe {
            (api.player_set_window)(player, surface.0)
        };
        #[cfg(not(windows))]
        unsafe {
            (api.player_set_window)(player, surface as u32)
        };

        let tools_instance = unsafe { (api.new)(args.len() as i32, args.as_ptr()) };
        let tools = (!tools_instance.is_null()).then(|| media_tools::MediaTools::new(api, tools_instance));

        Ok(Self { _lib: lib, api, instance, player, media: None, surface, tools })
    }

    pub fn surface(&self) -> Surface {
        self.surface
    }

    pub fn video_size(&self) -> Option<(u32, u32)> {
        let (mut w, mut h) = (0u32, 0u32);
        let result = unsafe { (self.api.video_get_size)(self.player, 0, &mut w, &mut h) };
        (result == 0 && w > 0 && h > 0).then_some((w, h))
    }

    pub fn tools(&self) -> Option<media_tools::MediaTools> {
        self.tools
    }

    pub fn open(&mut self, source: &str, start_ms: Option<i64>) -> Result<(), String> {
        unsafe { (self.api.player_stop)(self.player) };
        if let Some(old) = self.media.take() {
            unsafe { (self.api.media_release)(old) };
        }

        let is_url = source.starts_with("http://") || source.starts_with("https://");
        let c_source = CString::new(source).map_err(|e| e.to_string())?;
        let media = unsafe {
            if is_url {
                (self.api.media_new_location)(self.instance, c_source.as_ptr())
            } else {
                (self.api.media_new_path)(self.instance, c_source.as_ptr())
            }
        };
        if media.is_null() {
            return Err(tr!("libvlc não conseguiu abrir mídia: {source}", "libvlc couldn't open the media: {source}"));
        }
        if let Some(ms) = start_ms.filter(|ms| *ms > 0) {
            let option = CString::new(format!(":start-time={:.3}", ms as f64 / 1000.0)).map_err(|e| e.to_string())?;
            unsafe { (self.api.media_add_option)(media, option.as_ptr()) };
        }

        unsafe { (self.api.player_set_media)(self.player, media) };
        self.media = Some(media);
        Ok(())
    }

    pub fn play(&self) {
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

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct NowPlaying {
    pub title: String,
    pub episode_label: String,
    pub watch_id: Option<i64>,
    pub episode_number: Option<i64>,
    pub source: String,
    pub session: u64,
}

pub struct PlayerState {
    pub engine: std::sync::Mutex<Option<PlayerEngine>>,
    pub now_playing: std::sync::Mutex<NowPlaying>,
    #[cfg(any(windows, target_os = "linux"))]
    pub media_session: std::sync::Mutex<Option<media_session::MediaSession>>,
    pub video_size: std::sync::atomic::AtomicU64,
    pub video_hwnd: std::sync::atomic::AtomicIsize,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            engine: std::sync::Mutex::new(None),
            now_playing: std::sync::Mutex::new(NowPlaying::default()),
            video_size: std::sync::atomic::AtomicU64::new(0),
            video_hwnd: std::sync::atomic::AtomicIsize::new(0),
            #[cfg(any(windows, target_os = "linux"))]
            media_session: std::sync::Mutex::new(None),
        }
    }
}

impl PlayerState {
    #[cfg(any(windows, target_os = "linux"))]
    pub fn with_media_session(&self, f: impl FnOnce(&media_session::MediaSession)) {
        if let Some(session) = self.media_session.lock().unwrap().as_ref() {
            f(session);
        }
    }
}
