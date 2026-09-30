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

unsafe impl Send for MediaSession {}
unsafe impl Sync for MediaSession {}

impl MediaSession {
    pub fn new(app: &AppHandle, hwnd: HWND) -> windows::core::Result<Self> {
        let interop = factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
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

    pub fn set_playing(&self, playing: bool) {
        let status = if playing { MediaPlaybackStatus::Playing } else { MediaPlaybackStatus::Paused };
        let _ = self.smtc.SetPlaybackStatus(status);
    }
}

fn set_paused_from_key(app: &AppHandle, paused: bool) {
    let main_app = app.clone();
    let _ = app.run_on_main_thread(move || {
        let state = main_app.state::<super::PlayerState>();
        if let Some(engine) = state.engine.lock().unwrap().as_ref() {
            engine.set_paused(paused);
        }
        state.with_media_session(|s| s.set_playing(!paused));
    });
}
