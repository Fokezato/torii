//! Linux: controle de mídia via MPRIS (D-Bus) — teclas de mídia e o painel
//! de mídia do KDE/GNOME controlam o player e mostram o anime/episódio.
//! Mesma interface do `media_session` do Windows.
//!
//! Só fica registrado no D-Bus com algo aberto no player (`set_active`) —
//! fora disso as teclas continuam indo pros outros players.

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub struct MediaSession {
    app: AppHandle,
    controls: Mutex<MediaControls>,
}

impl MediaSession {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let controls = MediaControls::new(PlatformConfig { display_name: "Torii", dbus_name: "torii", hwnd: None })
            .map_err(|e| format!("{e:?}"))?;
        Ok(Self { app: app.clone(), controls: Mutex::new(controls) })
    }

    /// Liga ao abrir um episódio (com título no painel de mídia), desliga
    /// ao parar.
    pub fn set_active(&self, now_playing: Option<(&str, &str)>) {
        let mut controls = self.controls.lock().unwrap();
        match now_playing {
            Some((title, subtitle)) => {
                let app = self.app.clone();
                // Reanexar do zero: `attach` com sessão já ativa falha.
                let _ = controls.detach();
                if controls.attach(move |event| handle_event(&app, event)).is_err() {
                    return;
                }
                let _ = controls.set_metadata(MediaMetadata {
                    title: Some(title),
                    artist: Some(subtitle),
                    ..Default::default()
                });
                let _ = controls.set_playback(MediaPlayback::Playing { progress: None });
            }
            None => {
                let _ = controls.set_playback(MediaPlayback::Stopped);
                let _ = controls.detach();
            }
        }
    }

    pub fn set_playing(&self, playing: bool) {
        let playback =
            if playing { MediaPlayback::Playing { progress: None } } else { MediaPlayback::Paused { progress: None } };
        let _ = self.controls.lock().unwrap().set_playback(playback);
    }
}

fn handle_event(app: &AppHandle, event: MediaControlEvent) {
    match event {
        MediaControlEvent::Play => set_paused_from_key(app, Some(false)),
        MediaControlEvent::Pause => set_paused_from_key(app, Some(true)),
        MediaControlEvent::Toggle => set_paused_from_key(app, None),
        MediaControlEvent::Next => {
            let _ = app.emit("player:next-episode-requested", ());
        }
        _ => {}
    }
}

/// O libvlc é acionado na thread principal (dona da janela do vídeo), igual
/// aos comandos do player. `None` = alterna.
fn set_paused_from_key(app: &AppHandle, paused: Option<bool>) {
    let main_app = app.clone();
    let _ = app.run_on_main_thread(move || {
        let state = main_app.state::<super::PlayerState>();
        let paused = {
            let guard = state.engine.lock().unwrap();
            let Some(engine) = guard.as_ref() else { return };
            let paused = paused.unwrap_or_else(|| engine.snapshot().is_playing);
            engine.set_paused(paused);
            paused
        };
        state.with_media_session(|s| s.set_playing(!paused));
    });
}
