# Siegu Architecture & Documentation

Siegu is a cross-platform, privacy-first media backup and synchronization application built with Tauri, Rust, and Vue.js. It features local AI indexing for smart search and peer-to-peer (P2P) synchronization using WebRTC.

## Core Technology Stack

*   **Frontend:** Vue 3, Vuetify (UI framework), Vite.
*   **Backend:** Rust, Tauri.
*   **Database:** SQLite (via `rusqlite`).
*   **Machine Learning:** ONNX Runtime (via the `ort` crate) for local, offline inference (CLIP, UltraFace, etc.).
*   **Networking:** WebRTC (`webrtc-rs`) for P2P data transfer, WebSockets (`tokio-tungstenite`) for signaling.
*   **CLI:** `siegu` binary (via `clap`) for headless operation.

---

## Workspace Structure

```
siegu/
├── crates/
│   ├── siegu-core/      # Core library — no Tauri dependency
│   │   ├── src/
│   │   │   ├── config.rs          # Config validation and allowed keys
│   │   │   ├── database.rs        # SQLite database (photos, config, logs, people)
│   │   │   ├── error.rs           # SieguError enum (thiserror)
│   │   │   ├── event_bus.rs       # EventBus trait + implementations
│   │   │   ├── face_detector.rs   # UltraFace anchors, decode, NMS
│   │   │   ├── geocode.rs         # Offline reverse geocoding (embedded cities)
│   │   │   ├── lan_server.rs      # WebSocket signaling server (LAN)
│   │   │   ├── mdns.rs            # mDNS service discovery
│   │   │   ├── ml_worker.rs       # Job enum, model helpers, transaction wrapping
│   │   │   ├── model_manager.rs   # Model registry, SHA-256 verification, memory budget
│   │   │   ├── scanner.rs         # ScanGuard, EXIF extraction, extension checking
│   │   │   ├── server.rs          # Pairing code generation, SHA-256 hashing
│   │   │   ├── shutdown.rs        # ShutdownCoordinator (graceful shutdown)
│   │   │   ├── signal.rs          # SignalMessage enum (Go protocol)
│   │   │   ├── sync_transport.rs  # Filename sanitization, sync_temp cleanup
│   │   │   └── thumbnail.rs       # EXIF orientation, image/video thumbnails
│   │   └── Cargo.toml
│   ├── siegu-cli/       # CLI binary
│   │   ├── src/main.rs            # scan, analyze, models, sync, status, serve, config
│   │   └── Cargo.toml
│   └── siegu-tauri/     # Tauri app (src-tauri/)
├── src/                 # Vue 3 frontend
├── .github/workflows/ci.yml  # CI: test, clippy, cargo-audit, build, release
├── docker-compose.yml   # Go signaling server
├── ARCHITECTURE.md      # This file
└── Cargo.toml           # Workspace root
```

### Crate Dependencies

```
siegu-cli  ──→  siegu-core
siegu-tauri ──→  siegu-core
```

`siegu-core` has no Tauri dependency and can be used independently.

---

## 1. Peer-to-Peer Synchronization

The sync engine allows two devices to mirror their libraries without a cloud provider.

### A. Discovery & Handshake
1.  **Signaling:** Devices connect to a global signaling server via WebSockets using a shared `room_id` derived from a 4-word mnemonic or QR code.
2.  **WebRTC:** Peers exchange ICE candidates and SDP offers/answers to establish a direct, encrypted P2P connection.

### B. Sync Protocol (`SyncMessage`)
Once connected, devices communicate via a custom binary protocol over the WebRTC Data Channel:
1.  **Manifest Exchange:** One peer sends a `ManifestRequest`. The other responds with a `ManifestResponse` containing IDs and timestamps of all known photos.
2.  **Delta Calculation:** Peers compare manifests to identify missing files.
3.  **Chunked Transfer:** Missing files are requested via `FileRequest`. The sender transmits a `FileHeader`, followed by sequential 16KB `FileChunk` messages, and a `FileEnd` signal.
4.  **Database Integration:** Received files are stored in a `sync_temp` directory. Upon completion, they are moved to the authorized library folder and imported into the local SQLite database.

---

## 2. AI & Video Processing

### A. Image Indexing
*   **CLIP:** Generates 512-dimension feature vectors for semantic search (e.g., searching for "mountain" or "dog").
*   **UltraFace:** Detects human faces. Faces are cropped and stored locally for grouping.
*   **OCR, NSFW, Aesthetics, YOLO, BLIP, ArcFace, MiDaS, Whisper** — 14 models total.

### B. Video Analysis
*   Videos are processed by extracting keyframes at 1s, mid-point, and 90% duration.
*   Frames are analyzed by the ML worker. If a frame contains a face or a high-probability object, it is indexed against the video's ID, making the video searchable by its content.

### C. Model Management
*   **Model Registry:** 14 ONNX models with URLs, expected sizes, and SHA-256 hashes.
*   **Integrity Verification:** SHA-256 hash verification on download.
*   **Memory Budget:** Models are loaded on-demand; memory availability is checked before loading.
*   **Scan Deduplication:** `ScanGuard` prevents concurrent scans. `ScanSession` holds the lock.

---

## 3. Library & People Management

### A. Management Tools
*   **People Merging:** Users can merge two identified "People" into one (e.g., merging "Alice" and "Alice Smith"). The database updates all face relationships automatically.
*   **Rename:** Easily update the names of identified persons.
*   **Wipe Local Data:** When removing an authorized folder, users can choose to "Wipe Local Data," which removes all AI indexing, thumbnails, and metadata associated with that folder's files from the local database.

### B. Maintenance
*   **Database Cleanup:** Runs a `VACUUM` command on the SQLite database to reclaim space and optimize performance.
*   **Background Sync (Experimental):** Toggle to allow the app to maintain WebRTC connections when minimized (platform dependent).

---

## 4. Security & Privacy

*   **Local-First:** All AI processing happens on your device. No photos are ever uploaded to a server for indexing.
*   **End-to-End Encrypted:** WebRTC provides DTLS-encrypted P2P channels for all file transfers.
*   **Zero-Knowledge Discovery:** The signaling server only relays encrypted handshake data; it never sees your files or your library manifest.
*   **Model Integrity:** SHA-256 verification on all model downloads.
*   **Filename Sanitization:** All synced filenames are sanitized to prevent path traversal and control characters.
*   **Graceful Shutdown:** `ShutdownCoordinator` signals all background tasks on app exit.
*   **Config Validation:** Type-checked config keys with range constraints.
*   **Transaction Wrapping:** ML worker batch writes use BEGIN/COMMIT/ROLLBACK for crash safety.

---

## 5. CLI Usage

```bash
# Scan a folder
siegu scan /path/to/photos

# Check model status
siegu models list

# Download all models
siegu models download

# Show app status
siegu status

# Start LAN signaling server
siegu serve --port 8080

# Manage configuration
siegu config get
siegu config set theme dark
siegu config keys
```
