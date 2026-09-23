//! Bindings finas pro subconjunto da API C do libvlc que o player usa.
//!
//! Não usa os crates `vlc-rs`/`libvlc-sys` (abandonados desde 2018/2019, ver
//! discussão que motivou essa escolha) — a API C do libvlc é pequena e
//! estável há anos pra esse uso básico (abrir mídia, embutir em HWND,
//! play/pause/seek/volume), então declarar só o que precisa é mais robusto
//! que herdar 2 dependências mortas.
//!
//! Carregado via `libloading` (LoadLibrary em runtime) em vez de link
//! estático: o .dll oficial da VideoLAN não vem com `.lib` de import pra
//! MSVC, só o .dll em si — `libloading` contorna isso resolvendo cada
//! símbolo por nome depois de carregar.

use libloading::{Library, Symbol};
use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

pub type LibvlcInstance = c_void;
pub type LibvlcMedia = c_void;
pub type LibvlcMediaPlayer = c_void;
pub type LibvlcTime = i64;

type FnNew = unsafe extern "C" fn(c_int, *const *const c_char) -> *mut LibvlcInstance;
type FnRelease = unsafe extern "C" fn(*mut LibvlcInstance);
type FnMediaNewLocation = unsafe extern "C" fn(*mut LibvlcInstance, *const c_char) -> *mut LibvlcMedia;
type FnMediaNewPath = unsafe extern "C" fn(*mut LibvlcInstance, *const c_char) -> *mut LibvlcMedia;
type FnMediaRelease = unsafe extern "C" fn(*mut LibvlcMedia);
type FnPlayerNew = unsafe extern "C" fn(*mut LibvlcInstance) -> *mut LibvlcMediaPlayer;
type FnPlayerRelease = unsafe extern "C" fn(*mut LibvlcMediaPlayer);
type FnPlayerSetMedia = unsafe extern "C" fn(*mut LibvlcMediaPlayer, *mut LibvlcMedia);
type FnPlayerSetHwnd = unsafe extern "C" fn(*mut LibvlcMediaPlayer, *mut c_void);
type FnPlayerPlay = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> c_int;
type FnPlayerSetPause = unsafe extern "C" fn(*mut LibvlcMediaPlayer, c_int);
type FnPlayerStop = unsafe extern "C" fn(*mut LibvlcMediaPlayer);
type FnPlayerGetTime = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> LibvlcTime;
type FnPlayerSetTime = unsafe extern "C" fn(*mut LibvlcMediaPlayer, LibvlcTime) -> c_int;
type FnPlayerGetLength = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> LibvlcTime;
type FnPlayerIsPlaying = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> c_int;
type FnPlayerGetState = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> c_int;
type FnAudioGetVolume = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> c_int;
type FnAudioSetVolume = unsafe extern "C" fn(*mut LibvlcMediaPlayer, c_int) -> c_int;
type FnAudioGetTrack = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> c_int;
type FnAudioSetTrack = unsafe extern "C" fn(*mut LibvlcMediaPlayer, c_int) -> c_int;
type FnAudioGetTrackDescription = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> *mut LibvlcTrackDescription;
type FnSpuGet = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> c_int;
type FnSpuSet = unsafe extern "C" fn(*mut LibvlcMediaPlayer, c_int) -> c_int;
type FnSpuGetDescription = unsafe extern "C" fn(*mut LibvlcMediaPlayer) -> *mut LibvlcTrackDescription;
type FnTrackDescriptionListRelease = unsafe extern "C" fn(*mut LibvlcTrackDescription);
type FnMediaAddOption = unsafe extern "C" fn(*mut LibvlcMedia, *const c_char);
type FnMediaParseWithOptions = unsafe extern "C" fn(*mut LibvlcMedia, c_int, c_int) -> c_int;
type FnMediaGetParsedStatus = unsafe extern "C" fn(*mut LibvlcMedia) -> c_int;
type FnMediaGetDuration = unsafe extern "C" fn(*mut LibvlcMedia) -> LibvlcTime;
type FnMediaTracksGet = unsafe extern "C" fn(*mut LibvlcMedia, *mut *mut *mut LibvlcMediaTrack) -> u32;
type FnMediaTracksRelease = unsafe extern "C" fn(*mut *mut LibvlcMediaTrack, u32);
pub type VideoLockCb = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> *mut c_void;
pub type VideoUnlockCb = unsafe extern "C" fn(*mut c_void, *mut c_void, *const *mut c_void);
pub type VideoDisplayCb = unsafe extern "C" fn(*mut c_void, *mut c_void);
type FnVideoSetCallbacks = unsafe extern "C" fn(
    *mut LibvlcMediaPlayer,
    Option<VideoLockCb>,
    Option<VideoUnlockCb>,
    Option<VideoDisplayCb>,
    *mut c_void,
);
type FnVideoSetFormat = unsafe extern "C" fn(*mut LibvlcMediaPlayer, *const c_char, u32, u32, u32);

/// `libvlc_media_track_t` (libvlc 3.x). `u` é a union de ponteiros
/// (áudio/vídeo/legenda) — só lida como vídeo quando `i_type == 1`.
#[repr(C)]
pub struct LibvlcMediaTrack {
    pub i_codec: u32,
    pub i_original_fourcc: u32,
    pub i_id: c_int,
    /// -1 desconhecido, 0 áudio, 1 vídeo, 2 legenda.
    pub i_type: c_int,
    pub i_profile: c_int,
    pub i_level: c_int,
    pub u: *mut c_void,
    pub i_bitrate: u32,
    pub psz_language: *mut c_char,
    pub psz_description: *mut c_char,
}

/// Começo de `libvlc_video_track_t` — só os 2 primeiros campos são lidos.
#[repr(C)]
pub struct LibvlcVideoTrack {
    pub i_height: u32,
    pub i_width: u32,
}

/// Lista ligada (`p_next`) devolvida por `*_get_track_description`/
/// `video_get_spu_description` — 1 nó por faixa de áudio/legenda
/// disponível. `i_id` é o que `audio_set_track`/`video_set_spu` espera de
/// volta; `i_id == -1` é sempre "Desabilitado" (legenda off / áudio mudo),
/// já vem nessa lista, não precisa tratar à parte.
#[repr(C)]
pub struct LibvlcTrackDescription {
    pub i_id: c_int,
    pub psz_name: *mut c_char,
    pub p_next: *mut LibvlcTrackDescription,
}

/// Ponteiros resolvidos uma vez no load; `VlcApi` é só um bag de function
/// pointers `Copy`, sem lifetime pra carregar — o `Library` que os originou
/// fica vivo dentro de `PlayerEngine` (ver `mod.rs`) pelo tempo todo que a
/// API é usada, então os símbolos continuam válidos.
#[derive(Clone, Copy)]
pub struct VlcApi {
    pub new: FnNew,
    pub release: FnRelease,
    pub media_new_location: FnMediaNewLocation,
    pub media_new_path: FnMediaNewPath,
    pub media_release: FnMediaRelease,
    pub player_new: FnPlayerNew,
    pub player_release: FnPlayerRelease,
    pub player_set_media: FnPlayerSetMedia,
    pub player_set_hwnd: FnPlayerSetHwnd,
    pub player_play: FnPlayerPlay,
    pub player_set_pause: FnPlayerSetPause,
    pub player_stop: FnPlayerStop,
    pub player_get_time: FnPlayerGetTime,
    pub player_set_time: FnPlayerSetTime,
    pub player_get_length: FnPlayerGetLength,
    pub player_is_playing: FnPlayerIsPlaying,
    pub player_get_state: FnPlayerGetState,
    pub audio_get_volume: FnAudioGetVolume,
    pub audio_set_volume: FnAudioSetVolume,
    pub audio_get_track: FnAudioGetTrack,
    pub audio_set_track: FnAudioSetTrack,
    pub audio_get_track_description: FnAudioGetTrackDescription,
    pub spu_get: FnSpuGet,
    pub spu_set: FnSpuSet,
    pub spu_get_description: FnSpuGetDescription,
    pub track_description_list_release: FnTrackDescriptionListRelease,
    pub media_add_option: FnMediaAddOption,
    pub media_parse_with_options: FnMediaParseWithOptions,
    pub media_get_parsed_status: FnMediaGetParsedStatus,
    pub media_get_duration: FnMediaGetDuration,
    pub media_tracks_get: FnMediaTracksGet,
    pub media_tracks_release: FnMediaTracksRelease,
    pub video_set_callbacks: FnVideoSetCallbacks,
    pub video_set_format: FnVideoSetFormat,
}

macro_rules! sym {
    ($lib:expr, $name:literal) => {{
        let s: Symbol<'_, _> = $lib.get($name).map_err(|e| format!("símbolo {} não achado: {e}", stringify!($name)))?;
        *s
    }};
}

/// # Safety
/// `lib` precisa ser um `libvlc.dll` de verdade (ABI compatível) — chamar
/// isso contra qualquer outra DLL é UB no primeiro uso de um símbolo.
pub unsafe fn load(lib: &Library) -> Result<VlcApi, String> {
    Ok(VlcApi {
        new: sym!(lib, b"libvlc_new\0"),
        release: sym!(lib, b"libvlc_release\0"),
        media_new_location: sym!(lib, b"libvlc_media_new_location\0"),
        media_new_path: sym!(lib, b"libvlc_media_new_path\0"),
        media_release: sym!(lib, b"libvlc_media_release\0"),
        player_new: sym!(lib, b"libvlc_media_player_new\0"),
        player_release: sym!(lib, b"libvlc_media_player_release\0"),
        player_set_media: sym!(lib, b"libvlc_media_player_set_media\0"),
        player_set_hwnd: sym!(lib, b"libvlc_media_player_set_hwnd\0"),
        player_play: sym!(lib, b"libvlc_media_player_play\0"),
        player_set_pause: sym!(lib, b"libvlc_media_player_set_pause\0"),
        player_stop: sym!(lib, b"libvlc_media_player_stop\0"),
        player_get_time: sym!(lib, b"libvlc_media_player_get_time\0"),
        player_set_time: sym!(lib, b"libvlc_media_player_set_time\0"),
        player_get_length: sym!(lib, b"libvlc_media_player_get_length\0"),
        player_is_playing: sym!(lib, b"libvlc_media_player_is_playing\0"),
        player_get_state: sym!(lib, b"libvlc_media_player_get_state\0"),
        audio_get_volume: sym!(lib, b"libvlc_audio_get_volume\0"),
        audio_set_volume: sym!(lib, b"libvlc_audio_set_volume\0"),
        audio_get_track: sym!(lib, b"libvlc_audio_get_track\0"),
        audio_set_track: sym!(lib, b"libvlc_audio_set_track\0"),
        audio_get_track_description: sym!(lib, b"libvlc_audio_get_track_description\0"),
        spu_get: sym!(lib, b"libvlc_video_get_spu\0"),
        spu_set: sym!(lib, b"libvlc_video_set_spu\0"),
        spu_get_description: sym!(lib, b"libvlc_video_get_spu_description\0"),
        track_description_list_release: sym!(lib, b"libvlc_track_description_list_release\0"),
        media_add_option: sym!(lib, b"libvlc_media_add_option\0"),
        media_parse_with_options: sym!(lib, b"libvlc_media_parse_with_options\0"),
        media_get_parsed_status: sym!(lib, b"libvlc_media_get_parsed_status\0"),
        media_get_duration: sym!(lib, b"libvlc_media_get_duration\0"),
        media_tracks_get: sym!(lib, b"libvlc_media_tracks_get\0"),
        media_tracks_release: sym!(lib, b"libvlc_media_tracks_release\0"),
        video_set_callbacks: sym!(lib, b"libvlc_video_set_callbacks\0"),
        video_set_format: sym!(lib, b"libvlc_video_set_format\0"),
    })
}

/// `libvlc_state_t` (subconjunto relevante pro front — o resto vira "outro").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VlcState {
    Opening,
    Buffering,
    Playing,
    Paused,
    Stopped,
    Ended,
    Error,
    Other,
}

impl From<c_int> for VlcState {
    fn from(raw: c_int) -> Self {
        match raw {
            1 => VlcState::Opening,
            2 => VlcState::Buffering,
            3 => VlcState::Playing,
            4 => VlcState::Paused,
            5 => VlcState::Stopped,
            6 => VlcState::Ended,
            7 => VlcState::Error,
            _ => VlcState::Other,
        }
    }
}
