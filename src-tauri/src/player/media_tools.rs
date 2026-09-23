//! Info e quadros de arquivos de vídeo SEM passar pelo player principal —
//! usado pelo painel de episódios (miniatura, áudio/legenda, qualidade de
//! cada episódio) e pela miniatura da barra do tempo (hover estilo
//! YouTube). Instância própria do libvlc (ver `PlayerEngine::new`); o quadro
//! é decodificado direto pra memória (callbacks "vmem"), sem janela nem
//! áudio, e devolvido como JPEG.

use super::ffi::{LibvlcInstance, LibvlcMediaTrack, LibvlcVideoTrack, VlcApi, VlcState};
use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const FRAME_W: u32 = 384;
pub const FRAME_H: u32 = 216;

#[derive(Clone, Copy)]
pub struct MediaTools {
    api: VlcApi,
    instance: *mut LibvlcInstance,
}

// Instância do libvlc é thread-safe (doc da VideoLAN); o `Library` que dá
// vida aos ponteiros de `api` fica no `PlayerEngine`, vivo o app inteiro.
unsafe impl Send for MediaTools {}
unsafe impl Sync for MediaTools {}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProbeTrack {
    /// Código de idioma como vem do arquivo ("jpn", "por", "pt-BR"...).
    pub language: String,
    /// Nome que o release deu pra faixa ("Brazil", "Forced", "Signs"...).
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MediaProbe {
    pub duration_ms: i64,
    pub width: u32,
    pub height: u32,
    pub audio: Vec<ProbeTrack>,
    pub subtitles: Vec<ProbeTrack>,
}

/// Um grab por vez — cada um decodifica vídeo em software; em paralelo
/// (hover rápido na barra) só disputaria CPU com o player tocando.
static GRAB_LOCK: Mutex<()> = Mutex::new(());
static FRAME_CACHE: OnceLock<Mutex<HashMap<(String, i64), String>>> = OnceLock::new();
static PROBE_CACHE: OnceLock<Mutex<HashMap<String, MediaProbe>>> = OnceLock::new();
const FRAME_CACHE_MAX: usize = 400;

struct FrameSlot {
    buf_ptr: *mut u8,
    frames: Mutex<u32>,
    cv: Condvar,
}

unsafe extern "C" fn lock_cb(opaque: *mut c_void, planes: *mut *mut c_void) -> *mut c_void {
    let slot = &*(opaque as *const FrameSlot);
    *planes = slot.buf_ptr as *mut c_void;
    std::ptr::null_mut()
}

unsafe extern "C" fn unlock_cb(_opaque: *mut c_void, _picture: *mut c_void, _planes: *const *mut c_void) {}

unsafe extern "C" fn display_cb(opaque: *mut c_void, _picture: *mut c_void) {
    let slot = &*(opaque as *const FrameSlot);
    *slot.frames.lock().unwrap() += 1;
    slot.cv.notify_all();
}

fn cstr_or_empty(p: *const std::os::raw::c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

impl MediaTools {
    pub fn new(api: VlcApi, instance: *mut LibvlcInstance) -> Self {
        Self { api, instance }
    }

    pub fn release(&self) {
        unsafe { (self.api.release)(self.instance) };
    }

    pub fn probe_cached(&self, path: &str) -> Result<MediaProbe, String> {
        let cache = PROBE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(hit) = cache.lock().unwrap().get(path) {
            return Ok(hit.clone());
        }
        let probe = self.probe(path)?;
        cache.lock().unwrap().insert(path.to_string(), probe.clone());
        Ok(probe)
    }

    /// Lê duração, resolução e faixas de áudio/legenda do cabeçalho do
    /// arquivo (preparser do libvlc), sem decodificar vídeo.
    fn probe(&self, path: &str) -> Result<MediaProbe, String> {
        let api = self.api;
        let c_path = CString::new(path).map_err(|e| e.to_string())?;
        unsafe {
            let media = (api.media_new_path)(self.instance, c_path.as_ptr());
            if media.is_null() {
                return Err(format!("não conseguiu abrir {path}"));
            }
            // flags 0 = só local (sem rede/arte); timeout em ms.
            if (api.media_parse_with_options)(media, 0, 8000) != 0 {
                (api.media_release)(media);
                return Err("falha ao iniciar leitura do arquivo".to_string());
            }
            // 1 skipped, 2 failed, 3 timeout, 4 done — 0 = ainda lendo.
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut status = (api.media_get_parsed_status)(media);
            while status == 0 && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(25));
                status = (api.media_get_parsed_status)(media);
            }
            if status != 4 {
                (api.media_release)(media);
                return Err(format!("leitura do arquivo não concluiu (status {status})"));
            }

            let duration_ms = (api.media_get_duration)(media).max(0);
            let mut tracks: *mut *mut LibvlcMediaTrack = std::ptr::null_mut();
            let count = (api.media_tracks_get)(media, &mut tracks);
            let mut probe = MediaProbe { duration_ms, width: 0, height: 0, audio: Vec::new(), subtitles: Vec::new() };
            for i in 0..count as usize {
                let track = &**tracks.add(i);
                let entry = || ProbeTrack {
                    language: cstr_or_empty(track.psz_language),
                    description: cstr_or_empty(track.psz_description),
                };
                match track.i_type {
                    0 => probe.audio.push(entry()),
                    1 if !track.u.is_null() => {
                        let video = &*(track.u as *const LibvlcVideoTrack);
                        if video.i_width * video.i_height > probe.width * probe.height {
                            probe.width = video.i_width;
                            probe.height = video.i_height;
                        }
                    }
                    2 => probe.subtitles.push(entry()),
                    _ => {}
                }
            }
            if !tracks.is_null() {
                (api.media_tracks_release)(tracks, count);
            }
            (api.media_release)(media);
            Ok(probe)
        }
    }

    /// Quadro do vídeo em `time_ms` como data URL JPEG (cacheado por
    /// arquivo+tempo).
    pub fn frame_cached(&self, path: &str, time_ms: i64) -> Result<String, String> {
        let cache = FRAME_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let key = (path.to_string(), time_ms);
        if let Some(hit) = cache.lock().unwrap().get(&key) {
            return Ok(hit.clone());
        }
        let jpeg = {
            let _guard = GRAB_LOCK.lock().unwrap();
            self.grab_frame(path, time_ms)?
        };
        use base64::Engine;
        let url = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpeg));
        let mut cache = cache.lock().unwrap();
        if cache.len() >= FRAME_CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, url.clone());
        Ok(url)
    }

    fn grab_frame(&self, path: &str, time_ms: i64) -> Result<Vec<u8>, String> {
        let api = self.api;
        let c_path = CString::new(path).map_err(|e| e.to_string())?;
        let options: Vec<CString> = [
            ":no-audio".to_string(),
            ":no-spu".to_string(),
            ":no-osd".to_string(),
            // Decodificação por hardware entrega quadro em superfície de GPU
            // que precisaria ser copiada de volta — em software é direto.
            ":avcodec-hw=none".to_string(),
            format!(":start-time={:.3}", time_ms.max(0) as f64 / 1000.0),
        ]
        .into_iter()
        .map(|o| CString::new(o).unwrap())
        .collect();
        let chroma = CString::new("RV32").unwrap();

        let mut buf = vec![0u8; (FRAME_W * FRAME_H * 4) as usize];
        let slot = Box::new(FrameSlot { buf_ptr: buf.as_mut_ptr(), frames: Mutex::new(0), cv: Condvar::new() });
        let opaque = &*slot as *const FrameSlot as *mut c_void;

        let got = unsafe {
            let media = (api.media_new_path)(self.instance, c_path.as_ptr());
            if media.is_null() {
                return Err(format!("não conseguiu abrir {path}"));
            }
            for option in &options {
                (api.media_add_option)(media, option.as_ptr());
            }
            let player = (api.player_new)(self.instance);
            if player.is_null() {
                (api.media_release)(media);
                return Err("libvlc_media_player_new retornou nulo".to_string());
            }
            (api.player_set_media)(player, media);
            (api.video_set_callbacks)(player, Some(lock_cb), Some(unlock_cb), Some(display_cb), opaque);
            (api.video_set_format)(player, chroma.as_ptr(), FRAME_W, FRAME_H, FRAME_W * 4);
            (api.player_play)(player);

            // Espera o 2º quadro (o 1º logo depois do seek às vezes vem
            // cinza/incompleto); sai antes se o arquivo der erro/acabar.
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut frames = slot.frames.lock().unwrap();
            while *frames < 2 && Instant::now() < deadline {
                let (guard, _) = slot.cv.wait_timeout(frames, Duration::from_millis(100)).unwrap();
                frames = guard;
                if *frames == 0
                    && matches!(VlcState::from((api.player_get_state)(player)), VlcState::Error | VlcState::Ended)
                {
                    break;
                }
            }
            let got = *frames;
            drop(frames);

            // stop é síncrono: depois dele nenhum callback toca mais no
            // buffer/slot, dá pra ler e liberar com segurança.
            (api.player_stop)(player);
            (api.player_release)(player);
            (api.media_release)(media);
            got
        };
        drop(slot);

        if got == 0 {
            return Err("nenhum quadro decodificado".to_string());
        }
        let mut jpeg = Vec::new();
        jpeg_encoder::Encoder::new(&mut jpeg, 78)
            .encode(&buf, FRAME_W as u16, FRAME_H as u16, jpeg_encoder::ColorType::Bgra)
            .map_err(|e| e.to_string())?;
        Ok(jpeg)
    }
}
