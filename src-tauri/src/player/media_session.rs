//! Integração com o controle de mídia do Windows (SystemMediaTransportControls):
//! faz as teclas de mídia do teclado (play/pause, próxima) controlarem o
//! player e mostra o anime/episódio no painel de mídia do sistema. Sem
//! isso o libvlc embutido é invisível pro Windows — tecla de mídia não
//! pausava (bug real reportado).
//!
//! Só fica ATIVO com algo aberto no player (`set_active`) — fora disso as
//! teclas continuam indo pra outros apps (Spotify etc.).

use tauri::{AppHandle, Emitter, Manager};
use windows::core::{factory, HSTRING};
use windows::Foundation::TypedEventHandler;
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls, SystemMediaTransportControlsButton,
    SystemMediaTransportControlsButtonPressedEventArgs,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;

pub struct MediaSession {
    smtc: SystemMediaTransportControls,
}

// SMTC é objeto WinRT "agile" (pode ser usado de qualquer thread).
unsafe impl Send for MediaSession {}
unsafe impl Sync for MediaSession {}

impl MediaSession {
    /// `hwnd`: janela principal do app (o SMTC é associado a ela).
    pub fn new(app: &AppHandle, hwnd: HWND) -> windows::core::Result<Self> {
        let interop = factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
        // SAFETY: hwnd é a janela principal, viva o app inteiro.
        let smtc: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd)? };
        smtc.SetIsPlayEnabled(true)?;
        smtc.SetIsPauseEnabled(true)?;
        smtc.SetIsNextEnabled(true)?;
        smtc.SetIsEnabled(false)?;

        let app = app.clone();
        smtc.ButtonPressed(&TypedEventHandler::<
            SystemMediaTransportControls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(move |_, args| {
            let Some(args) = args.as_ref() else { return Ok(()) };
            let button = args.Button()?;
            // Play/pause direto no motor (funciona mesmo com a janela de
            // controles sem foco); "próximo" precisa da lista de episódios,
            // que a overlay tem — vai por evento.
            if button == SystemMediaTransportControlsButton::Play {
                set_paused_from_key(&app, false);
            } else if button == SystemMediaTransportControlsButton::Pause {
                set_paused_from_key(&app, true);
            } else if button == SystemMediaTransportControlsButton::Next {
                let _ = app.emit("player:next-episode-requested", ());
            }
            Ok(())
        }))?;
        Ok(Self { smtc })
    }

    /// Liga ao abrir um episódio (com título no painel do Windows), desliga
    /// ao parar — aí as teclas de mídia voltam pros outros apps.
    pub fn set_active(&self, now_playing: Option<(&str, &str)>) {
        let _ = (|| -> windows::core::Result<()> {
            match now_playing {
                Some((title, subtitle)) => {
                    let updater = self.smtc.DisplayUpdater()?;
                    updater.SetType(MediaPlaybackType::Video)?;
                    let props = updater.VideoProperties()?;
                    props.SetTitle(&HSTRING::from(title))?;
                    props.SetSubtitle(&HSTRING::from(subtitle))?;
                    updater.Update()?;
                    self.smtc.SetPlaybackStatus(MediaPlaybackStatus::Playing)?;
                    self.smtc.SetIsEnabled(true)?;
                }
                None => {
                    self.smtc.SetPlaybackStatus(MediaPlaybackStatus::Stopped)?;
                    self.smtc.SetIsEnabled(false)?;
                }
            }
            Ok(())
        })();
    }

    /// O Windows decide se a tecla play/pause manda "Play" ou "Pause" por
    /// esse status — tem que acompanhar o estado real do player.
    pub fn set_playing(&self, playing: bool) {
        let status = if playing { MediaPlaybackStatus::Playing } else { MediaPlaybackStatus::Paused };
        let _ = self.smtc.SetPlaybackStatus(status);
    }
}

fn set_paused_from_key(app: &AppHandle, paused: bool) {
    let state = app.state::<super::PlayerState>();
    if let Some(engine) = state.engine.lock().unwrap().as_ref() {
        engine.set_paused(paused);
    }
    state.with_media_session(|s| s.set_playing(!paused));
}
