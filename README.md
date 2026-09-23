# Torii

An anime hub for Windows: it follows the seasons you watch, downloads new episodes automatically and plays everything in its own player, no torrent client or external player needed.

> The app interface is currently in Brazilian Portuguese. English is planned.

## Features

- **Catalog** with the current season, trending shows and search (data from AniList).
- **Library** grouped by anime, with all seasons side by side and "download another season" right from the anime page.
- **Automatic downloads** of new episodes (built-in torrent engine), filtered by quality, audio and subtitle language.
- **Native player**, Netflix style:
  - resumes where you left off;
  - picks your preferred audio and subtitle language automatically;
  - skips openings, endings and recaps (data from AniSkip), with an option to jump straight to the next episode;
  - thumbnail previews on the seek bar, episode panel and YouTube-style keyboard shortcuts;
  - works with the Windows media keys.
- **Disk space savings** (optional): delete after watching, strip unused audio tracks and downscale to 720p.
- **Jellyfin integration** (optional): delivers episodes straight to your library folder.
- System tray, notifications and start with Windows.

## Requirements

- Windows 10 or 11 (64-bit)
- [Microsoft Edge WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on Windows 11)

## Building from source

Requires [Node.js](https://nodejs.org/) 20+ and [Rust](https://rustup.rs/) (stable).

```bash
npm install
npm run tauri dev     # development
npm run tauri build   # installer in src-tauri/target/release/bundle
```

## Stack

[Tauri 2](https://tauri.app/) (Rust) + React, TypeScript and Tailwind CSS.

## Third-party components

- **libVLC** ([VideoLAN](https://www.videolan.org/vlc/libvlc.html)), LGPL-2.1: video playback. Shipped in this repository under `src-tauri/vendor/vlc` with its original license (`COPYING.txt`). Torii loads it dynamically and does not modify it.
- **FFmpeg** ([BtbN LGPL builds](https://github.com/BtbN/FFmpeg-Builds)), LGPL: used only by the file size reduction options. It is **not** shipped: the app downloads it when you enable one of those options and verifies it against the published SHA-256.
- **librqbit**: torrent engine.
- Data from [AniList](https://anilist.co/), [AniSkip](https://aniskip.com/) and [Nyaa](https://nyaa.si/).

## Disclaimer

Torii does not host, distribute or store any media files. It is a manager that automates searching and downloading torrents over public protocols (BitTorrent) and organizes what you download yourself. You are solely responsible for the content you obtain. Respect the copyright laws of your country.

## License

[PolyForm Noncommercial 1.0.0](LICENSE): you may use, study, modify and distribute Torii and modified versions, as long as it is **not for commercial purposes**. Selling it, charging for a subscription or premium features, or adding ads is not allowed, in the original or in modified versions.

The third-party components above are covered by their own licenses.
