<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="Torii logo" width="128" height="128">
</p>

<h1 align="center">Torii</h1>

<p align="center">
  <b>Your anime hub for Windows.</b><br>
  Follow the seasons you watch, get new episodes downloaded automatically<br>
  and watch everything in a built-in player. No torrent client or external player needed.
</p>

<p align="center">
  <a href="https://github.com/Fokezato/torii/releases"><img src="https://img.shields.io/github/v/release/Fokezato/torii?include_prereleases&label=release&color=FF6A45" alt="Latest release"></a>
  <a href="https://github.com/Fokezato/torii/releases"><img src="https://img.shields.io/github/downloads/Fokezato/torii/total?color=FF6A45" alt="Downloads"></a>
  <img src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D6?logo=windows&logoColor=white" alt="Platform: Windows 10 | 11">
  <img src="https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white" alt="Built with Tauri 2">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-PolyForm%20Noncommercial-6C7180" alt="License: PolyForm Noncommercial"></a>
</p>

<p align="center">
  <a href="#-features">Features</a> ·
  <a href="#-installation">Installation</a> ·
  <a href="#%EF%B8%8F-keyboard-shortcuts">Shortcuts</a> ·
  <a href="#%EF%B8%8F-building-from-source">Building</a> ·
  <a href="#-license">License</a>
</p>

> [!NOTE]
> Torii is in **beta**, and the app interface is currently in **Brazilian Portuguese**. English is planned.

## ✨ Features

- 📚 **Catalog.** Current season, trending shows and search, powered by AniList.
- 🗂️ **Library grouped by anime.** All seasons side by side, with "download another season" right from the anime page.
- ⬇️ **Automatic downloads.** New episodes are fetched by the built-in torrent engine, filtered by quality, audio and subtitle language.
- ▶️ **Native player, Netflix style.**
  - Resumes where you left off.
  - Picks your preferred audio and subtitle language automatically.
  - Skips openings, endings and recaps (powered by AniSkip), and can jump straight to the next episode.
  - Thumbnail previews on the seek bar and an episode panel.
  - YouTube-style keyboard shortcuts and Windows media keys.
- 💾 **Disk space savings** (optional). Delete episodes after watching, strip unused audio tracks and downscale to 720p.
- 🎬 **Jellyfin integration** (optional). Delivers episodes straight to your Jellyfin library folder.
- 🔔 **Lives in the tray.** Notifications, background checks and start with Windows.

## 🚀 Installation

1. Download **`Torii_x.y.z_x64-setup.exe`** from the [latest release](https://github.com/Fokezato/torii/releases).
2. Run the installer.

> [!IMPORTANT]
> The installer is not code-signed yet, so Windows shows a blue **"Windows protected your PC"** screen. Click **More info → Run anyway**.

**Requirements:** Windows 10 or 11 (64-bit). [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) is required and comes preinstalled on Windows 11; the installer downloads it if it's missing.

## ⌨️ Keyboard shortcuts

| Key | Action |
| --- | --- |
| <kbd>K</kbd> / <kbd>Space</kbd> | Play / pause |
| <kbd>J</kbd> / <kbd>L</kbd> | Back / forward 10 seconds |
| <kbd>←</kbd> / <kbd>→</kbd> | Back / forward 5 seconds |
| <kbd>↑</kbd> / <kbd>↓</kbd> | Volume up / down |
| <kbd>M</kbd> | Mute |
| <kbd>F</kbd> | Fullscreen |
| <kbd>Esc</kbd> | Exit fullscreen |
| <kbd>0</kbd> – <kbd>9</kbd> | Jump to 0% – 90% of the episode |
| <kbd>Shift</kbd> + <kbd>N</kbd> | Next episode |

The Windows media keys (play/pause, next) work too, even when Torii isn't focused.

## 🛠️ Building from source

Requires [Node.js](https://nodejs.org/) 20+ and [Rust](https://rustup.rs/) (stable).

```bash
git clone https://github.com/Fokezato/torii.git
cd torii
npm install
npm run tauri dev     # run in development
npm run tauri build   # build the installer (src-tauri/target/release/bundle)
```

**Stack:** [Tauri 2](https://tauri.app/) (Rust) · React · TypeScript · Tailwind CSS

## 🧩 Third-party components

| Component | Used for | License |
| --- | --- | --- |
| [libVLC](https://www.videolan.org/vlc/libvlc.html) (VideoLAN) | Video playback. Shipped in `src-tauri/vendor/vlc` with its original license, loaded dynamically and unmodified. | LGPL-2.1 |
| [FFmpeg](https://github.com/BtbN/FFmpeg-Builds) (BtbN LGPL builds) | File size reduction options. **Not** shipped: downloaded on demand when you enable one of them, and verified against the published SHA-256. | LGPL |
| [librqbit](https://github.com/ikatson/rqbit) | Torrent engine | Apache-2.0 |
| [AniList](https://anilist.co/) · [AniSkip](https://aniskip.com/) · [Nyaa](https://nyaa.si/) | Anime data, skip times and episode search | — |

## ⚖️ Disclaimer

Torii does not host, distribute or store any media files. It is a manager that automates searching and downloading torrents over public protocols (BitTorrent) and organizes what you download yourself. You are solely responsible for the content you obtain. Respect the copyright laws of your country.

## 📄 License

Torii is licensed under the [PolyForm Noncommercial License 1.0.0](LICENSE).

You may use, study, modify and share Torii and modified versions, as long as it is **not for commercial purposes**. Selling it, charging for a subscription or premium features, or adding ads is not allowed, in the original or in modified versions.

The third-party components above are covered by their own licenses.
