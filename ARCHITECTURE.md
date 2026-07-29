# Siegu Architecture

Siegu is a cross-platform, privacy-first media library with local AI indexing and peer-to-peer synchronization. All ML runs on-device; no cloud or telemetry.

## Quick Overview

| Layer | Technology |
|-------|-----------|
| Frontend | Vue 3, Vuetify 3, Vite, TypeScript |
| Desktop Shell | Tauri v2 (Rust) |
| Mobile Shell | Tauri Android (Kotlin + WebView) |
| Core Library | Rust (siegu-core) |
| Database | SQLite via rusqlite |
| ML Runtime | ONNX Runtime via ort |
| P2P Networking | WebRTC via webrtc-rs |
| Signaling | WebSockets via tokio-tungstenite |

## Crate Dependency Graph

```
siegu-cli ──→ siegu-core
siegu-tauri ──→ siegu-core
```

`siegu-core` has zero Tauri dependency and can be used standalone (CLI).

## Detailed Documentation

All documentation has moved to `docs/`:

### User-Facing

| Doc | Description |
|-----|-------------|
| [Getting Started](docs/getting-started.md) | Prerequisites, install, first run, basic usage |
| [Build from Source](docs/build.md) | Building for desktop, Android, iOS |
| [Configuration](docs/configuration.md) | All config keys reference |
| [CLI Reference](docs/cli.md) | CLI commands with examples |
| [Mesh Sync Guide](docs/sync.md) | How to sync devices |

### Technical

| Doc | Description |
|-----|-------------|
| [Architecture](docs/architecture.md) | Workspace layout, module map, feature flags |
| [Database Schema](docs/database.md) | All 12 tables, columns, indexes |
| [ML Engine](docs/ml-engine.md) | Models, preprocessing, pipeline |
| [Mesh Protocol](docs/mesh-protocol.md) | Signaling, WebRTC, sync protocol |
| [Frontend](docs/frontend.md) | Stores, composables, events |
| [Backend](docs/backend.md) | Tauri commands, state, plugins |
| [Android](docs/android.md) | Build pipeline, permissions |
| [iOS](docs/ios.md) | Build setup |
| [Developing](docs/developing.md) | Dev setup, githooks, CI, debugging |
| [Security](docs/security.md) | Privacy model, encryption |
