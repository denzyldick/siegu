# Siegu

[![License: FSL-1.1-Apache-2.0](https://img.shields.io/badge/License-FSL--1.1--Apache--2.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/built%20with-Tauri-blueviolet)](https://tauri.app/)
[![Vue.js](https://img.shields.io/badge/frontend-Vue.js%203-4fc08d)](https://vuejs.org/)
[![Rust](https://img.shields.io/badge/backend-Rust-000000)](https://www.rust-lang.org/)

**Siegu** (pronounced *see-goo*) is a privacy-first, local-only media management application. It organizes, secures, and synchronizes your photo and video library across devices without ever touching the cloud.

![Siegu Screenshot](./branding/screenshot.png)

## Key Features

### Privacy-First AI
- **Local Semantic Search**: Find photos by describing them (e.g., "sunset at the beach") using local **CLIP** models.
- **Face Recognition**: Automatically detect and group faces using **UltraFace**, all processed offline.
- **14 AI Models**: CLIP, UltraFace, OCR, NSFW, Aesthetics, YOLO, BLIP, ArcFace, MiDaS, Whisper, SAM, SuperRes.
- **Zero Cloud**: No telemetry, no tracking, and no external AI API calls.

### Peer-to-Peer Synchronization
- **Cloudless Sync**: Mirror your library between devices using encrypted **WebRTC** data channels.
- **Mnemonic Discovery**: Connect devices using a 4-word mnemonic or QR code -- no accounts required.
- **Delta Transfers**: Only sync what's missing with intelligent manifest comparison.
- **Go Signaling Server**: Docker-based signaling server (`ghcr.io/denzyldick/signalling-server`).

### Smart Library Management
- **Watched Folders**: Monitor directories for real-time library updates.
- **EXIF Extraction**: 22 EXIF properties extracted and displayed (camera, lens, GPS, orientation).
- **Optimized Thumbnails**: Orientation-correct thumbnails stored in a local SQLite database.
- **Video Indexing**: Keyframe extraction and AI analysis make video content searchable.

### CLI & Daemon Mode
- **`siegu` CLI**: Full headless access -- scan, analyze, manage models, sync, serve.
- **Workspace Architecture**: `siegu-core` (shared library), `siegu-cli`, `siegu-tauri` (GUI).
- **Zero Code Duplication**: All business logic in `siegu-core`, used by both CLI and GUI.

## Design Philosophy

- **Minimalist Aesthetic**: Pure black interactive elements on a clean white/zinc background.
- **Tactile Feedback**: Every interaction features smooth scaling and transitions.
- **Consistency**: A unified "Button + Icon" language across the entire application.

## Getting Started

### Prerequisites
- **Node.js** (v18+)
- **Rust** (Latest Stable)
- **System Dependencies**: See [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites)

### Installation

```bash
git clone https://github.com/denzyldick/siegu.git
cd siegu
npm install
npm run tauri dev
```

### Neovim Debugging

If you want to use Neovim with `nvim-dap` and `codelldb`, this repo includes a
project-local config in [`.nvim.lua`](./.nvim.lua).

Enable local config loading in your own Neovim setup:

```lua
vim.o.exrc = true
vim.o.secure = true
```

Then open the repo root in Neovim and use:

```vim
:SieguTauriDev
:SieguDebugAttach
```

Workflow:

1. Run `:SieguTauriDev` to start the Tauri dev session.
2. Wait for the app window to open.
3. Run `:SieguDebugAttach` to attach CodeLLDB to the Rust backend and step through code.

Requirements:

- `nvim-dap`
- `codelldb` on your `PATH`
- Neovim with local `exrc` enabled

### Android Build & Deploy

Build and install the Android app on a connected device:

```bash
bash scripts/run-android.sh
```

The script:
1. Builds the frontend (`yarn build`)
2. Cross-compiles the Rust library for `aarch64-linux-android` via `cargo-ndk`
3. Copies `libsiegu_lib.so` into `jniLibs/arm64-v8a`
4. Builds a universal debug APK via Gradle
5. Installs it on the connected device via `adb install -r`

**Prerequisites**: Android SDK + NDK r27 (`~/Android/Sdk`), `cargo-ndk`, Rust target `aarch64-linux-android`, connected device/emulator.

### CLI Installation

```bash
cargo install --path crates/siegu-cli
siegu --help
```

## CLI Usage

```bash
# Scan a folder for media
siegu scan /path/to/photos

# Check model status
siegu models list

# Download all AI models
siegu models download

# Show app status (photos, config, models, memory)
siegu status

# Start LAN signaling server
siegu serve --port 8080

# Manage configuration
siegu config get
siegu config set theme dark
siegu config set tier paid
siegu config keys
```

## Tech Stack

- **Frontend**: Vue 3, Vuetify 3, Vite
- **Core**: Rust, Tauri v2
- **Database**: SQLite (rusqlite)
- **AI Engine**: ONNX Runtime (ort)
- **Networking**: WebRTC (webrtc-rs), WebSockets (tokio-tungstenite)
- **CLI**: clap, indicatif, tracing
- **Signaling**: Go server (Docker)

## Workspace Structure

```
siegu/
├── crates/
│   ├── siegu-core/      # Core library (16 modules, 130 tests)
│   ├── siegu-cli/       # CLI binary (scan, models, config, serve)
│   └── siegu-tauri/     # Tauri GUI app
├── src/                 # Vue 3 frontend
├── .github/workflows/   # CI (test, clippy, cargo-audit, build)
├── docker-compose.yml   # Go signaling server
└── ARCHITECTURE.md      # Detailed architecture docs
```

## Security

- **Model Integrity**: SHA-256 verification on all model downloads.
- **Filename Sanitization**: Synced filenames sanitized against path traversal.
- **Graceful Shutdown**: Background tasks shut down cleanly on app exit.
- **Transaction Safety**: ML batch writes use BEGIN/COMMIT/ROLLBACK.
- **Config Validation**: Type-checked config keys with range constraints.
- **Scan Deduplication**: Concurrent scan prevention via `ScanGuard`.

## License

Siegu is licensed under the **Functional Source License, Version 1.1 (FSL-1.1-Apache-2.0)**.

- **Commercial Use**: Allowed for non-competing products.
- **Competing Use**: Prohibited until the **Change Date**.
- **Change Date**: **March 9, 2028** (at which point the license automatically becomes **Apache 2.0**).

See the [LICENSE](LICENSE) file for full details.

---

Built with love by [Denzyl Dick](https://github.com/denzyldick)
