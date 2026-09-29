//! Janela nativa onde o libvlc desenha o vídeo, por cima da área do
//! `<video-slot>` do React — o WebView não aceita vídeo nativo "dentro" do
//! DOM. Cada sistema tem a sua implementação com a mesma interface:
//! Windows (HWND filha, `windows_impl`) e Linux (janela X11 filha,
//! `linux_impl`). `Surface` é o handle nativo; `to_raw`/`from_raw` guardam
//! ele num atômico (ver `PlayerState::video_hwnd`).

#[cfg(windows)]
mod windows_impl;
#[cfg(windows)]
pub use windows_impl::*;

#[cfg(target_os = "linux")]
mod linux_impl;
#[cfg(target_os = "linux")]
pub use linux_impl::*;
