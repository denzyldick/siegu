# Getting Started

## About Siegu

**Siegu** (pronounced *see-goo*) is a privacy-first, local-only media manager for your photo and video library. It scans the folders you choose, builds a searchable library entirely on your machine, and never uploads your media anywhere. Every AI feature runs on-device via ONNX Runtime — no cloud, no accounts, no telemetry.

### What Siegu can do

- **Organize** — Automatic scanning, EXIF metadata extraction, thumbnails, video indexing, and a map view with heatmaps.
- **Semantic search** — Describe a photo in your own words ("sunset at the beach") and CLIP finds it. Text search also matches file names, recognized text, object tags, captions, and locations.
- **Discover** — The search dropdown shows rails at a glance: Best shots, Favorites, Recent, People, Locations, Tags, Papers & screenshots, Cameras, and Months, plus one-tap filters (Favorites, Videos, Faces, Papers, NSFW, Surprise me).
- **People** — Automatic face detection and grouping. Name people, merge duplicates, and jump to every photo of a person.
- **Analyze** — On-device models enrich each photo with captions (BLIP), objects (YOLO), recognized text (OCR), aesthetics scores, depth maps (MiDaS), and audio transcription for videos (Whisper).
- **Sync** — Encrypted peer-to-peer sync between your own devices over the network (WebRTC), with mesh networking via LAN discovery, QR codes, or mnemonic phrases.

### Privacy model

Everything happens on your computer. There are no accounts, no analytics, no cloud uploads. AI models are downloaded once from public sources (e.g. HuggingFace) and then run locally forever after. See [docs/security.md](security.md) for details.

---

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

The app window opens with an onboarding wizard that walks you through setup.

---

## First Run

1. **Welcome**: The wizard opens with a greeting. Choose **Set up locally** or **Set up with sync** (to link another device)
2. **Folders**: Click "Add Directory" to point at your photo/video library
3. **AI models**: Download at least two models (the Continue button unlocks once two are ready)
4. **Sync (optional)**: Link a device to keep libraries in sync across devices, or skip
5. **Scan**: The scanner finds media, extracts EXIF metadata, and generates thumbnails
6. **Analyze**: With models downloaded, run analysis to index faces, captions, objects, etc.

> **Tip**: Scanning builds the base library immediately. AI enrichment (search quality, faces, captions) improves as models finish analyzing your photos — you can keep using the app while it runs.

---

## Using the app

### The library

Your photos and videos appear as a scrollable grid, grouped by month. The toolbar lets you:

- **Sort** — newest, oldest, best (aesthetics score), or surprise (random)
- **Filter** — favorites, videos, faces, papers & screenshots, NSFW, and a date range
- **Search** — plain text or semantic ("a group of friends at a beach")

Click any item to open the viewer, where you can favorite it, copy recognized text (OCR), or see its AI metadata.

### Discovery (the search dropdown)

Click the search bar to open "Your library at a glance":

- **Best shots** — the highest-rated photo for each day, based on aesthetics scores
- **Magic cards** — one-tap filters: Favorites, Videos, Faces, Papers & screenshots, NSFW (only shown when NSFW content exists), and Surprise me
- **People** — named people and unnamed face groups to browse
- **Locations / Tags / Papers / Cameras / Months** — browse the library by these facets

Filters stack: you can combine e.g. *Faces* + a specific *month* to narrow down results.

### People

Siegu groups faces automatically. Open the **People** page to:

- **Name** a person — new names create a person; you can also pick an existing person to merge into
- **Manage** — rename or merge duplicate people
- **Browse** — every photo containing that person, with unnamed face groups shown separately for later naming

### Sync

Use the **Connect** panel to host or join a sync session with another of your devices:

- **LAN discovery** — devices on the same network find each other automatically
- **QR code / mnemonic phrase** — connect over any network
- Syncs photos, videos, and edits between devices, encrypted end-to-end

See [docs/sync.md](sync.md) for the full guide.

### Configuration

Language, theme, directories, models, and sync settings live under **Settings**. All preferences are stored locally in `siegu.db`. See [docs/configuration.md](configuration.md).

### CLI

Everything above is also available headless via the `siegu` CLI — scanning, model downloads, analysis, and mesh hosting. See [docs/cli.md](cli.md).

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
