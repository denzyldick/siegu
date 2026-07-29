# Architecture

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Frontend | Vue 3, Vuetify 3, Vite, TypeScript |
| Desktop Shell | Tauri v2 (Rust) |
| Mobile Shell | Tauri Android (Kotlin + WebView) |
| Core Library | Rust (siegu-core) |
| Database | SQLite via rusqlite |
| ML Runtime | ONNX Runtime via ort crate |
| Tokenization | HuggingFace tokenizers |
| P2P Networking | WebRTC via webrtc-rs |
| Signaling | WebSockets via tokio-tungstenite |
| LAN Discovery | mDNS via mdns-sd |
| Video Processing | ffmpeg-next (optional, video-thumbs feature) |

## Crate Dependency Graph

```
siegu-cli ──→ siegu-core
siegu-tauri ──→ siegu-core
```

`siegu-core` has zero Tauri dependency and can be used standalone.

## Workspace Layout

```
siegu/
├── crates/
│   ├── siegu-core/            # Core library — no Tauri dependency
│   │   ├── src/
│   │   │   ├── lib.rs         # Module declarations + public exports
│   │   │   ├── config.rs      # Config validation (22 keys)
│   │   │   ├── database.rs    # SQLite: 12 tables
│   │   │   ├── error.rs       # SieguError enum (7 variants)
│   │   │   ├── event_bus.rs   # EventBus trait variants
│   │   │   ├── face_detector.rs # UltraFace anchor math + NMS
│   │   │   ├── geocode.rs     # Offline reverse geocoding
│   │   │   ├── lan_server.rs  # Warp WebSocket signaling
│   │   │   ├── mdns.rs        # mDNS service registration
│   │   │   ├── mesh.rs        # Sync: Manifest, FileChunk, SyncEvent trait
│   │   │   ├── mesh_transport.rs # WebRTC + WebSocket transport
│   │   │   ├── ml_engine/     # ML inference pipeline
│   │   │   ├── ml_worker.rs   # ML job queue
│   │   │   ├── model_manager.rs # Model registry (21 files, 10 groups)
│   │   │   ├── scanner.rs     # EXIF extraction, ScanGuard
│   │   │   ├── server.rs      # BIP39 pairing codes
│   │   │   ├── shutdown.rs    # ShutdownCoordinator
│   │   │   ├── signal.rs      # SignalMessage enum (17 variants)
│   │   │   ├── sync_transport.rs # Filename sanitization
│   │   │   └── thumbnail.rs   # 320px base64 JPEG thumbnails
│   │   └── Cargo.toml
│   │
│   ├── siegu-cli/             # CLI binary (clap)
│   │   └── src/
│   │       ├── main.rs        # 7 command groups
│   │       └── analyze_tui.rs # Ratatui progress viewer
│   │
├── src-tauri/                 # Tauri desktop/mobile app
│   ├── src/
│   │   ├── main.rs            # Entry point
│   │   ├── lib.rs             # Setup: 13 plugins, 50 commands, 5 state
│   │   ├── common.rs          # Config path, logging
│   │   ├── transport.rs       # Transport factory
│   │   ├── tauri_sync_event.rs # SyncEvent → Tauri events bridge
│   │   ├── ml.rs              # ML worker bridge
│   │   ├── file.rs            # File watcher, scanner
│   │   ├── wallpaper_plugin.rs # Android wallpaper
│   │   ├── geocode.rs         # Geocode command
│   │   ├── commands/          # 11 command modules
│   │   └── vendor/            # Vendored dependencies
│   └── gen/android/           # Android Gradle project
│
├── src/                       # Vue 3 frontend
│   ├── composables/           # 7 composables
│   ├── stores/                # 6 Pinia stores
│   ├── components/            # 50+ Vue components
│   ├── types/                 # Type definitions
│   ├── locales/               # 8 languages
│   └── services/              # Tauri IPC wrappers
│
├── scripts/
│   └── run-android.sh         # Android build + deploy
│
├── .github/workflows/         # CI pipelines
├── docker-compose.yml         # Go signaling server
└── Cargo.toml                 # Workspace root
```

## Feature Flags (siegu-core)

| Feature | Deps | Purpose |
|---------|------|---------|
| `ml` (default) | `ort`, `tokenizers` | AI model inference |
| `video-thumbs` | `ffmpeg-next` | Video keyframe extraction |
