# Getting Started

## Prerequisites

- **Node.js** v18+
- **Rust** latest stable (install via [rustup](https://rustup.rs/))
- **System dependencies** (varies per platform)

### Linux (Ubuntu/Debian)

```bash
sudo apt-get install -y \
  pkg-config libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev \
  libsoup-3.0-dev libjs-mathjax build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libavformat-dev libavcodec-dev libavutil-dev libswscale-dev \
  libswresample-dev libavfilter-dev libavdevice-dev libpostproc-dev
```

### macOS

```bash
brew install ffmpeg
```

### Windows

- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) with "Desktop development with C++" workload
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (included in Windows 11)
- FFmpeg via vcpkg: `vcpkg install ffmpeg:x64-windows`

---

## Quick Install

```bash
git clone https://github.com/denzyldick/siegu.git
cd siegu
npm install
npm run tauri dev
```

The app window opens with an onboarding wizard. Select your photo directories and start scanning.

---

## First Run

1. **Onboarding**: Choose your language and theme
2. **Add folders**: Click "Add Directory" to point at your photo/video library
3. **Scan**: The scanner finds media, extracts EXIF metadata, and generates thumbnails
4. **AI models**: Download models via Settings → Models (or `siegu models download`)
5. **Analyze**: After models are downloaded, run analysis to index faces, captions, objects, etc.

---

## Basic Usage

| Task | GUI | CLI |
|------|-----|-----|
| Scan folders | Settings → Directories → Add | `siegu scan /path/to/photos` |
| Download models | Settings → Models → Download | `siegu models download` |
| Analyze photos | Auto after scan (if indexing_mode != manual) | `siegu analyze all` |
| Search | Search bar (text or semantic) | — |
| Mesh sync | Connect panel → Host or Join | `siegu mesh host` / `siegu mesh join <room>` |
| Change config | Settings → Preferences | `siegu config set <key> <val>` |
