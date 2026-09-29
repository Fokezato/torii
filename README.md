<p align="center">
  <img src=".github/ToriiBanner.png" alt="Torii" width="640">
</p>

<p align="center">
  <b>Your anime hub.</b><br>
  Follow the seasons you watch, get new episodes downloaded automatically, or stream them on demand,<br>
  and watch everything in a built-in player. No torrent client or external player needed.
</p>

<p align="center">
  <a href="https://github.com/Fokezato/torii/releases"><img src="https://img.shields.io/github/v/release/Fokezato/torii?include_prereleases&label=release&color=FF6A45" alt="Latest release"></a>
  <a href="https://github.com/Fokezato/torii/releases"><img src="https://img.shields.io/github/downloads/Fokezato/torii/total?color=FF6A45" alt="Downloads"></a>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%26%20Android%20soon-0078D6" alt="Platform: Windows | Linux &amp; Android soon">
  <img src="https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white" alt="Built with Tauri 2">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-PolyForm%20Noncommercial-6C7180" alt="License: PolyForm Noncommercial"></a>
</p>

<p align="center">
  <a href="#-features">Features</a> ·
  <a href="#-requirements">Requirements</a> ·
  <a href="#-installation">Installation</a> ·
  <a href="#%EF%B8%8F-building-from-source">Building</a> ·
  <a href="#-license">License</a>
</p>

> [!NOTE]
> Torii is in **beta** and may still have bugs or rough edges. If you run into a problem, please [open an issue](https://github.com/Fokezato/torii/issues).
>
> Available for Windows today; Linux and Android versions are planned.

## ✨ Features

- 📚 **Catalog and search.** Browse the current season and trending shows, or search any title, powered by AniList.
- 🗂️ **Organized library.** Every anime in one place, with all of its seasons grouped together.
- ⬇️ **Automatic downloads.** New episodes are found and downloaded as soon as they are released, matching your preferred quality and language. Stalled downloads switch to another source on their own.
- 📡 **Streaming mode.** Nothing is downloaded ahead of time: press play and the episode starts right away while it downloads. The next episode is prepared halfway through, and watched episodes are deleted afterwards. Episodes that are still downloading can also be watched right away.
- ▶️ **Native video player.**
  - Lightweight and optimized, with resume and next episode.
  - Skips openings, endings and recaps, using AniSkip, the file's chapters or audio matching between episodes.
  - Ambient light around the video and customizable subtitles.
- 💾 **Storage management** (optional, beta). Delete episodes after watching, remove unused audio tracks or reduce resolution to save disk space.
- 🎬 **Jellyfin integration** (optional). Sends downloaded episodes straight to your Jellyfin library folder.
- 🎮 **Discord Rich Presence** (optional). Shows what you're watching on your Discord profile.
- 🔄 **Automatic updates.** New versions are offered inside the app.

## 💻 Requirements

- Windows 10 or 11 (64-bit)
- [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) runtime (preinstalled on Windows 11; the installer downloads it if missing)
- Internet connection for the catalog and downloads

## 🚀 Installation

1. Download **`Torii_x.y.z_x64-setup.exe`** from the [latest release](https://github.com/Fokezato/torii/releases).
2. Run the installer.

From version 0.4.0 on, Torii updates itself: when a new version is released, a banner inside the app offers to install it.

> [!IMPORTANT]
> The installer is not code-signed yet, so Windows shows a blue **"Windows protected your PC"** screen. Click **More info → Run anyway**.

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
| [FFmpeg](https://github.com/BtbN/FFmpeg-Builds) (BtbN LGPL builds) | File size reduction, custom subtitles, faster opening of some MKV files and opening/ending detection. **Not** shipped: downloaded on demand the first time a feature needs it, and verified against the published SHA-256. | LGPL |
| [librqbit](https://github.com/ikatson/rqbit) | Torrent engine and streaming | Apache-2.0 |
| [discord-rich-presence](https://github.com/vionya/discord-rich-presence) | Discord Rich Presence | MIT |
| [AniList](https://anilist.co/) · [AniSkip](https://aniskip.com/) · [Nyaa](https://nyaa.si/) | Anime data, skip times and episode search | — |

## ⚖️ Disclaimer

Torii does not host, distribute or store any media files. It is a manager that automates searching and downloading torrents over public protocols (BitTorrent) and organizes what you download yourself. You are solely responsible for the content you obtain. Respect the copyright laws of your country.

## 📄 License

Torii is licensed under the [PolyForm Noncommercial License 1.0.0](LICENSE).

You may use, study, modify and share Torii and modified versions, as long as it is **not for commercial purposes**. Selling it, charging for a subscription or premium features, or adding ads is not allowed, in the original or in modified versions.

The third-party components above are covered by their own licenses.
