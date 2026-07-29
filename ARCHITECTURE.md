# Siegu Architecture

Siegu is a cross-platform, privacy-first media library with local AI indexing and peer-to-peer synchronization. All ML runs on-device; no cloud or telemetry.

---

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
| Video Processing | ffmpeg-next (optional feature) |
| Face Detection | UltraFace (ONNX) |
| Object Detection | YOLOv8n (ONNX) |
| Image Captioning | BLIP (ONNX, encoder + decoder) |
| Audio Transcription | Whisper tiny (ONNX, encoder + decoder) |
| Depth Estimation | MiDaS (ONNX) |
| Face Recognition | ArcFace (ONNX) |
| Semantic Search | CLIP ViT-B/32 (ONNX, visual + text) |

---

## Workspace Layout

```
siegu/
├── crates/
│   ├── siegu-core/            # Core library — no Tauri dependency
│   │   ├── src/
│   │   │   ├── lib.rs         # Module declarations + public exports
│   │   │   ├── config.rs      # Config validation (23 keys)
│   │   │   ├── database.rs    # SQLite: 12 tables, photo/face/device storage
│   │   │   ├── error.rs       # SieguError enum (7 variants)
│   │   │   ├── event_bus.rs   # EventBus trait (null/tracing/callback/arc variants)
│   │   │   ├── face_detector.rs # UltraFace anchor math + NMS
│   │   │   ├── geocode.rs     # Offline reverse geocoding (250 embedded cities)
│   │   │   ├── lan_server.rs  # Warp WebSocket signaling server (rooms + relay)
│   │   │   ├── mdns.rs        # mDNS: register/unregister/discover/watch_hosts
│   │   │   ├── mesh.rs        # Sync protocol: Manifest, File Chunking, SyncEvent trait
│   │   │   ├── mesh_transport.rs # WebRTC + WebSocket transport layer
│   │   │   ├── ml_engine/     # ML inference pipeline (subdirectory)
│   │   │   │   ├── mod.rs     # Re-exports
│   │   │   │   ├── ep.rs      # ONNX execution provider (CUDA/DML/CoreML/CPU)
│   │   │   │   ├── models.rs  # LoadedModels: 18 optional model handles
│   │   │   │   ├── pipeline.rs # Photo analysis pipeline (10 models)
│   │   │   │   ├── preprocessing.rs # Image preprocessing per model
│   │   │   │   ├── whisper.rs # Audio transcription + mel spectrogram
│   │   │   │   └── worker.rs  # Background job processor + AnalysisCallbacks trait
│   │   │   ├── ml_worker.rs   # ML job queue + model helpers
│   │   │   ├── model_manager.rs # Model registry (21 files, 10 groups)
│   │   │   ├── scanner.rs     # EXIF extraction, media detection, ScanGuard
│   │   │   ├── server.rs      # BIP39 pairing codes + SHA-256 hashing
│   │   │   ├── shutdown.rs    # ShutdownCoordinator (watch channel)
│   │   │   ├── signal.rs      # SignalMessage enum (17 signaling variants)
│   │   │   ├── sync_transport.rs # Filename sanitization + temp cleanup
│   │   │   └── thumbnail.rs   # Thumbnail generation (320px base64 JPEG)
│   │   └── Cargo.toml
│   │
│   ├── siegu-cli/             # CLI binary
│   │   ├── src/
│   │   │   ├── main.rs        # Clap CLI: scan, analyze, models, serve, mesh, config
│   │   │   └── analyze_tui.rs # Ratatui progress viewer
│   │   └── Cargo.toml
│   │
├── src-tauri/                 # Tauri desktop/mobile app
│   ├── src/
│   │   ├── main.rs            # Entry point → siegu_lib::run()
│   │   ├── lib.rs             # Tauri setup: 13 plugins, 50 commands, 5 managed states
│   │   ├── common.rs          # get_config_path(), emit_log()
│   │   ├── transport.rs       # create_transport() factory, media server
│   │   ├── tauri_sync_event.rs # TauriSyncEvent: SyncEvent → Tauri events bridge
│   │   ├── ml.rs              # start_background_worker() bridge
│   │   ├── file.rs            # File watcher, scanner, base64 reader
│   │   ├── thumbnail.rs       # Thumbnail Tauri command
│   │   ├── wallpaper_plugin.rs # Android wallpaper plugin registration
│   │   ├── geocode.rs         # Tauri geocode command
│   │   ├── commands/          # 11 command modules (scan, models, photos, sync, etc.)
│   │   └── vendor/            # Vendored crates (see below)
│   └── gen/android/           # Android Gradle project (see Android section)
│
├── src/                       # Vue 3 frontend
│   ├── composables/           # 7 composables (useConnect, useSettings, usePeople, etc.)
│   ├── stores/                # 5 Pinia stores (app, ui, search, sync, scan, models)
│   ├── components/            # 50+ Vue components in 8 subdirectories
│   ├── types/                 # 7 type definition files
│   ├── services/              # Tauri IPC wrappers
│   └── locales/               # 8 languages (en, nl, fr, de, es, it, pt, pap)
│
├── scripts/
│   └── run-android.sh         # Android build + deploy script
│
├── docker-compose.yml         # Go signaling server container
├── ARCHITECTURE.md            # This file
└── Cargo.toml                 # Workspace root
```

### Crate Dependency Graph

```
siegu-cli ──→ siegu-core
siegu-tauri ──→ siegu-core
```

`siegu-core` has zero Tauri dependency and can be used standalone (e.g., from the CLI).

---

## Database Schema

File: `crates/siegu-core/src/database.rs`

12 tables for photos, AI results, devices, and configuration.

### `photo` — Main media table

| Column | Type | Notes |
|--------|------|-------|
| `id` | STRING PK | |
| `location` | STRING | UNIQUE index |
| `encoded` | STRING | Base64 320px thumbnail |
| `created` | DATE_TIME | Indexed |
| `latitude` | REAL | Spatial index with longitude |
| `longitude` | REAL | Spatial index with latitude |
| `indexed` | INTEGER | 0=new, 1=metadata, 2=fully processed |
| `caption` | TEXT | BLIP-generated caption |
| `aesthetics_score` | REAL | Aesthetics model score |
| `favorite` | INTEGER | Default 0 |
| `sync_needed` | INTEGER | Default 0 |

### `ai_status` — Per-photo model processing flags

One column per model: `clip`, `face`, `ocr`, `nsfw`, `aesthetics`, `yolo`, `blip`, `arcface`, `midas`, `whisper`, `sam`, `superres`. All `INTEGER DEFAULT 0`. FK `photo_id`.

### `faces` — Detected faces

| Column | Type | Notes |
|--------|------|-------|
| `face_id` | STRING PK | UUID |
| `photo_id` | STRING | FK to photo |
| `crop_path` | STRING | Filesystem path to cropped face |
| `encoded` | STRING | Base64 thumbnail |
| `embedding` | BLOB | 512xf32 ArcFace embedding |
| `person_id` | STRING | FK to people.id |

### `people` — Person identities

| Column | Type | Notes |
|--------|------|-------|
| `id` | STRING PK | UUID |
| `name` | STRING | Nullable (null = unnamed) |
| `embedding` | BLOB | 512xf32 centroid |

### `peer_device` — Mesh-connected devices

| Column | Type | Notes |
|--------|------|-------|
| `device_id` | TEXT PK | |
| `name` | TEXT | |
| `ip` | TEXT | |
| `port` | INTEGER | |
| `device_type` | TEXT | |
| `os` | TEXT | |
| `models_enabled` | TEXT | JSON array |
| `protocol_version` | INTEGER | Default 1 |
| `storage_used` / `storage_capacity` | INTEGER | |
| `last_seen` | TEXT | |

### Other tables

- **`ocr`**: photo_id → text
- **`directory`**: monitored folder paths
- **`object`**: photo_id → class + probability
- **`properties`**: photo_id → key/value pairs (EXIF, location, transcript, favorite)
- **`device`**: legacy paired device storage
- **`config`**: key/value app configuration
- **`logs`**: app log entries (timestamp, level, message)

---

## ML Engine

The ML pipeline lives in `crates/siegu-core/src/ml_engine/`. It runs all models via ONNX Runtime with CPU fallback, and optional CUDA/DirectML/CoreML providers.

### Model Registry

21 model files across 10 model groups, defined in `model_manager.rs`:

| Model | Files | Size | Purpose |
|-------|-------|------|---------|
| **clip** | visual, text, tokenizer.json | 190MB | Semantic search embeddings |
| **ultraface** | version-RFB-320.onnx | 1MB | Face detection |
| **ocr** | det, rec, en_dict.txt | 4MB | PP-OCRv4 text recognition |
| **nsfw** | nsfw.onnx | 10MB | Sensitive content detection |
| **aesthetics** | aesthetics.onnx | 10MB | Photo quality scoring |
| **yolo** | yolov8.onnx | 10MB | 80-class object detection |
| **blip** | encoder, decoder, tokenizer.json | 980MB | Image captioning |
| **arcface** | arcface.onnx | 10MB | Face recognition (512-dim) |
| **midas** | midas.onnx | 100MB | Depth estimation |
| **whisper** | encoder, decoder, tokenizer.json | 150MB | Audio transcription |

### Analysis Pipeline (`pipeline.rs`)

For each photo, `analyze_single_photo()` runs enabled models sequentially:
1. **CLIP** — 512-dim embedding for semantic search
2. **UltraFace** — face bounding boxes → ArcFace embeddings → DB storage
3. **OCR** — text detection → recognition
4. **NSFW** — binary classification
5. **Aesthetics** — score 1-10
6. **YOLO** — 80 COCO classes, filtered at 0.5 confidence
7. **BLIP** — greedy autoregressive caption (max 20 tokens, 384×384 input)
8. **ArcFace** — embedding extraction for detected faces
9. **MiDaS** — depth map
10. **Whisper** — audio transcription (video only)

Results are flushed to the SQLite database in batches with transaction wrapping.

### Preprocessing (`preprocessing.rs`)

Each model has its own image preprocessing function with model-specific dimensions and normalization:

| Model | Input Size | Normalization |
|-------|-----------|---------------|
| CLIP | 224×224 | ImageNet mean/std |
| Aesthetics | 384×384 | [-1, 1] |
| NSFW | 224×224 | ImageNet mean/std |
| OCR | 320×48 (det) / varies (rec) | [0, 1] |
| YOLO | 640×640 | [0, 1] |
| BLIP | 384×384 | ImageNet mean/std |
| MiDaS | 256×256 | [0, 1] |
| ArcFace | 112×112 | [-1, 1] |
| Face detection | 320×240 | [-1, 1] |

### Audio Transcription (`whisper.rs`)

- Extracts Log-Mel spectrogram (80 mel bins, 3000 frames)
- Runs Whisper tiny ONNX encoder → decoder loop
- Greedy token decoding with BOS/EOS handling
- Video frame extraction via ffmpeg (1s, mid, 90% keyframes)

---

## Mesh Synchronization

The sync system enables multi-device library mirroring without any cloud infrastructure.

### Discovery

1. **mDNS** (`mdns.rs`): Hosts register `_siegu._tcp.local.` services with `protocol_version` and `room_id` in TXT records. Joiners discover hosts via `discover_hosts()` (timeout-based scan) or `watch_hosts()` (continuous channel-based monitoring).
2. **QR / Mnemonic** (`server.rs`): BIP39 generates a 6-word passphrase + UUID. SHA-256 hashes into a `room_id`. Host displays QR code; joiner scans or types the phrase.
3. **Manual**: Direct IP + port entry for LAN hosts.

### Signaling

**LAN mode**: `lan_server.rs` starts a warp WebSocket server on a random port. Manages rooms as `HashMap<String, Room>` with up to 5 clients per room (`MAX_MESH_DEVICES`). Relays Offer/Answer/ICE candidates between peers. Self-contained — no external server needed.

**Remote mode**: Connects to a Go signaling server (`wss://siegu.io/ws` or self-hosted via `docker-compose.yml`).

### Signaling Protocol (`signal.rs`)

17 message types over WebSocket JSON:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `Join` | Client → Server | Join a room with device_id |
| `Joined` | Server → Client | Confirmation + peer_count |
| `PeerJoined` | Server → Client | Another device joined |
| `PeerDisconnected` | Server → Client | Peer left |
| `Offer` | Peer → Peer | WebRTC SDP offer |
| `Answer` | Peer → Peer | WebRTC SDP answer |
| `IceCandidate` | Peer → Peer | ICE candidate |
| `Relay` | Peer → Peer | Relay through connected peer |
| `RoomClosed` | Server → Client | Room destroyed |

### Transport (`mesh_transport.rs`)

`MeshTransport` wraps a WebSocket connection to the signaling server and manages WebRTC peer connections:

1. Connects to signaling WebSocket
2. Sends `Join` with room_id + device metadata
3. Receives `Joined` confirmation (peer_count indicates if others are present)
4. When another peer is present or joins, initiates WebRTC handshake
5. Creates RTCDataChannel for sync messages
6. Receiving end muxes incoming messages to `MeshManager::handle_sync_message()`
7. Supports relay mode through intermediate peers (mesh topology)

### Sync Protocol (`mesh.rs`)

Once a WebRTC data channel is established, `SyncMessage` enum drives the protocol:

| Message | Purpose |
|---------|---------|
| `ManifestRequest` / `ManifestResponse` | Exchange photo inventory |
| `FileRequest` | Request specific photo by ID |
| `FileHeader` | Metadata before transfer |
| `FileChunk` | 64KB chunk of file data |
| `FileEnd` | Signal transfer complete |
| `StartSync` | Trigger sync session |
| `MetadataUpdate` | Propagate AI results (caption, score) |
| `VersionNegotiate` / `VersionReject` | Protocol version handshake |
| `CatchUp` / `CatchUpDone` | Incremental sync |

**File transfer**: 64KB chunks with retry logic. Received files land in `sync_temp/`, verified, then moved to the library folder and imported via `import_photo()`.

**Storage quota**: Configurable via `max_storage_mb` config key. Enforced pre-receive in `FileHeader` handler by walking sync directory size.

---

## Frontend Architecture

### State Management

6 Pinia stores in `src/stores/`:
- **`app`**: Initialization state, onboarding completion
- **`ui`**: Theme (light/dark/system), sidebar visibility, current view
- **`search`**: Query, results, filters (favorites, videos, date range), pagination
- **`sync`**: Sync progress, peers, connection status
- **`scan`**: Scan progress, file counts
- **`models`**: Model download status, progress

### Composables

7 composables in `src/composables/` for reusable logic:
- **`useConnect`**: Host/join connection state machine, mDNS discovery, WebRTC event listeners
- **`useSettings`**: Config read/write via Tauri commands
- **`usePeople`**: Face/person CRUD operations
- **`useLocale`**: i18n locale switching
- **`useMediaUtils`**: Media filtering (videos-only, favorites)
- **`useMediaUrl`**: Resolves thumbnail/original URLs via media server
- **`useTauri`**: Typed invoke wrapper

### Event System

Tauri events bridge Rust backend → Vue frontend (defined in `src/types/events.ts`):

| Event | Payload | Source |
|-------|---------|--------|
| `scan-progress` | `ScanProgress` | File scanner |
| `indexing-progress` | `IndexingProgress` | ML worker |
| `indexing-eta` | string | ML worker |
| `sync-progress` | `SyncProgress` | Mesh transport |
| `webrtc-state` | string | TauriSyncEvent |
| `peer-connected` | `PeerDevice` | TauriSyncEvent |
| `peer-disconnected` | string (peer_id) | TauriSyncEvent |
| `room-code` | string | Pairing code |
| `photo-analysis-result` | Photo ID | ML worker |
| `model-progress` | `ModelProgress` | Downloader |
| `download-progress` | `DownloadProgress` | Model download |
| `sync-error` | string | Mesh transport |
| `photo-received` | `Photo` | TauriSyncEvent |

### Connection Flow (`useConnect.ts`)

1. User selects **Host** or **Join** mode via `ConnectModeToggle`
2. **Host**: Generates pairing codes, starts LAN signaling server, registers mDNS service, shows QR + passphrase
3. **Join**: Discovers LAN hosts via mDNS polling, or scans QR / enters passphrase
4. After WebRTC handshake, `peer-connected` fires → peer list updates
5. Sync progress flows via `sync-progress` events

---

## Tauri Integration

File: `src-tauri/src/lib.rs`

### Managed State

5 shared states managed by Tauri:

| State | Type | Purpose |
|-------|------|---------|
| `WebRtcState` | `sync_tx`, `active_session` | WebRTC session + sync channel |
| `ScanState` | `is_scanning` | Prevents concurrent scans |
| `MdnsState` | `daemon` | mDNS daemon handle |
| `ShutdownState` | `coordinator` | Graceful shutdown signal |
| `MlContext` | `tx`, `pending_count`, `abort` | ML worker control |
| `MediaServerState` | `port` | Media HTTP server port |

### Tauri Commands

~50 commands across 11 modules in `src-tauri/src/commands/`:

| Module | Commands |
|--------|----------|
| **sync** | start_webrtc_session, start_lan_host, stop_webrtc_session, discover_lan_devices, join_network, remove_device, list_devices, request_start_sync, initialize_sync_folder, get_media_server_port, generate_pairing_codes, hash_pairing_code, auto_reconnect, clear_saved_session, list_peer_devices |
| **photos** | list_files, toggle_favorite, get_photo_by_id, get_photo_encoded_batch, get_photos_for_map_click, get_heatmap_data |
| **scan** | scan_files |
| **models** | check_models, download_models |
| **indexing** | get_indexing_status, get_unindexed_count, index_faces, analyze_photo, analyze_photo_model, analyze_model, abort_indexing |
| **people** | get_people, get_unnamed_faces, assign_name_to_face, get_person_photos, get_person_faces, get_faces_for_photo, delete_face, get_top_tags, merge_people, rename_person |
| **directories** | add_directory, list_directories, remove_directory, remove_directory_full, is_initialized |
| **config** | save_config, get_config, get_os |
| **geocode** | list_objects, resolve_photo_locations, get_location_names |
| **logging** | get_logs, clear_logs, get_last_scan_time, cleanup_database |
| **wallpaper** | set_wallpaper |

### Plugins

13 Tauri plugins registered:
- `tauri-plugin-updater` — App updates
- `tauri-plugin-notification` — System notifications
- `tauri-plugin-os` — OS info
- `tauri-plugin-fs` — Filesystem access
- `tauri-plugin-dialog` — File dialogs
- `tauri-plugin-opener` — Open files with system handler
- `tauri-plugin-shell` — Shell commands
- `tauri-plugin-process` — Process management
- `tauri-plugin-global-shortcut` — Keyboard shortcuts (desktop)
- `tauri-plugin-clipboard-manager` — Clipboard (desktop)
- `tauri-plugin-wallpaper` — Wallpaper setting (Android)

---

## Android Build

### Structure

```
src-tauri/gen/android/
├── app/
│   ├── build.gradle.kts
│   └── src/main/
│       ├── AndroidManifest.xml
│       ├── java/io/denzyl/siegu/
│       │   ├── MainActivity.kt
│       │   ├── WallpaperPlugin.kt
│       │   └── generated/     # Tauri auto-generated bindings
│       └── res/               # Icons, themes, strings
├── buildSrc/                  # Custom Gradle Rust build plugin
├── build.gradle.kts
├── settings.gradle
└── gradle.properties
```

### Permissions (AndroidManifest.xml)

- `INTERNET`, `ACCESS_NETWORK_STATE`, `ACCESS_WIFI_STATE` — Networking
- `CAMERA` — QR scanning (optional feature, not required)
- `READ_EXTERNAL_STORAGE`, `WRITE_EXTERNAL_STORAGE` — Legacy storage
- `READ_MEDIA_IMAGES`, `READ_MEDIA_VIDEO`, `READ_MEDIA_AUDIO` — Scoped storage
- `MANAGE_EXTERNAL_STORAGE` — Full file access
- `ACCESS_FINE_LOCATION`, `ACCESS_COARSE_LOCATION` — GPS for photo geotagging
- `ACCESS_MEDIA_LOCATION` — EXIF GPS reading
- `NEARBY_WIFI_DEVICES` — mDNS LAN discovery
- `SET_WALLPAPER` — Set photo as wallpaper

### Build Pipeline

`scripts/run-android.sh` automates the full build:

1. `yarn build` — Build Vue frontend
2. `cargo ndk -t aarch64-linux-android build --release` — Cross-compile Rust
3. Copy `libsiegu_lib.so` → `jniLibs/arm64-v8a/`
4. `./gradlew assembleUniversalDebug` — Build universal APK
5. `adb install -r` — Install on connected device

**Prerequisites**: Android SDK + NDK r27, Rust target `aarch64-linux-android`, `cargo-ndk`.

---

## Vendored Dependencies

### esaxx-rs

**Location**: `src-tauri/vendor/esaxx-rs/`

**Purpose**: Rust wrapper around SentencePiece's esaXX suffix array C++ library. It's a transitive dependency of the `tokenizers` crate (used by the ML engine for CLIP/BLIP/Whisper tokenization).

**Why vendored**: The crates.io version `v0.1.10` had a C runtime (CRT) mismatch on Windows CI. The vendored copy patches this. The workspace `Cargo.toml` uses `[patch.crates-io]` to override:

```toml
[patch.crates-io]
esaxx-rs = { path = "src-tauri/vendor/esaxx-rs" }
```

On Linux/macOS, the crates.io version works fine. The vendored copy is only necessary for Windows builds.

---

## CLI

The `siegu` binary provides headless access to all core functionality:

```bash
# Scanning
siegu scan /path/to/photos          # Add folder to library
siegu analyze all                   # Run all ML models
siegu analyze photo <id>            # Single photo analysis
siegu analyze model <model_id>      # Run specific model on all photos

# Models
siegu models list                   # Show download status
siegu models download [names...]    # Download models
siegu models usage                  # Disk usage per model

# Mesh sync
siegu mesh host                     # Start LAN host with mDNS
siegu mesh join <room_id>           # Join mesh room
siegu mesh status                   # Session status
siegu mesh disconnect               # Leave mesh
siegu mesh quota                    # Storage quota info

# Server
siegu serve --port 8080             # Standalone signaling server

# Config
siegu config get                    # All config values
siegu config set <key> <val>        # Set config
siegu config keys                   # List valid keys

# Status
siegu status                        # Library, models, config summary
```

The CLI uses `CliSyncEvent` (implements `SyncEvent`) for output. `analyze_tui.rs` provides a ratatui-based real-time progress viewer for analysis operations.

---

## Config Reference

23 validated config keys defined in `config.rs`:

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `theme` | string | `system` | light/dark/system |
| `scan_threads` | integer | `4` | Parallel scan threads |
| `indexing_mode` | string | `immediate` | immediate/idle/manual |
| `tier` | string | `free` | free/paid |
| `max_storage_mb` | integer | `0` (unlimited) | Sync storage quota |
| `language` | string | `en` | UI language |
| `disable_models` | string | (empty JSON) | Disabled model list |
| `exclude_models` | string | (empty JSON) | Excluded model list |
| `session_port` | integer | `0` | LAN session port |
| `session_room_id` | string | `` | Saved session room |
| `session_signaling_url` | string | `` | Saved session URL |
| `session_is_initiator` | bool | `false` | Saved session role |
| `session_passphrase` | string | `` | Saved session passphrase |

---

## Error Handling

`SieguError` enum (`error.rs`) with 7 variants:

| Variant | Context |
|---------|---------|
| `Database` | SQLite operations |
| `Io` | File system operations |
| `Network` | WebSocket / WebRTC failures |
| `Config` | Invalid config values |
| `Model` | ONNX model loading or inference |
| `Scan` | Concurrent scan prevention |
| `Sync` | Protocol or transfer errors |
| `Shutdown` | Graceful shutdown signaling |

All commands return `Result<T, String>` for Tauri compatibility, mapping internal errors to user-facing messages.

---

## Security & Privacy

- **Local-First AI**: All 14 models run on-device via ONNX Runtime. No data ever leaves your computer.
- **Zero Telemetry**: No analytics, crash reporting, or usage tracking.
- **DTLS Encryption**: WebRTC data channels are encrypted with DTLS for all peer-to-peer transfers.
- **Zero-Knowledge Signaling**: The signaling server only relays encrypted SDP/ICE data. It never sees files or manifests.
- **SHA-256 Verification**: All model downloads are verified against expected hashes.
- **Filename Sanitization**: Synced filenames are sanitized to prevent path traversal and control characters.
- **Transaction Safety**: ML batch writes use BEGIN/COMMIT/ROLLBACK. Database VACUUM for maintenance.
- **Graceful Shutdown**: `ShutdownCoordinator` signals all background tasks (scan, sync, ML) before exit.
- **Config Validation**: All config keys are type-checked with range constraints. Invalid values are rejected.
