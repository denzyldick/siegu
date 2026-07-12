# siegu-core

Core library for [Siegu](https://github.com/denzyldick/siegu) — privacy-first media management.

This crate provides all business logic without any Tauri dependency. It can be used by the CLI, the Tauri app, or any other Rust project.

## Modules

| Module | Description |
|---|---|
| `config` | Config key validation with type constraints |
| `database` | SQLite database (photos, config, logs, people, faces) |
| `error` | `SieguError` enum (thiserror) |
| `event_bus` | EventBus trait + Null/Tracing/Callback/Arc implementations |
| `face_detector` | UltraFace anchors, decode, NMS |
| `geocode` | Offline reverse geocoding (embedded world cities) |
| `lan_server` | WebSocket signaling server for LAN sync |
| `mdns` | mDNS service discovery |
| `ml_worker` | Job enum, model helpers, pending count, transaction wrapping |
| `model_manager` | Model registry (14 ONNX models), SHA-256 verification, memory budget |
| `scanner` | ScanGuard (deduplication), EXIF extraction, extension checking |
| `server` | Pairing code generation with 6-word passphrase |
| `shutdown` | ShutdownCoordinator for graceful shutdown |
| `signal` | SignalMessage enum (Go signaling server protocol) |
| `sync_transport` | Filename sanitization, sync_temp cleanup |
| `thumbnail` | EXIF orientation, image/video thumbnails |

## Usage

```rust
use siegu_core::config::validate_config_value;
use siegu_core::model_manager::check_models_downloaded;
use siegu_core::scanner::is_media_file;

// Validate config
assert!(validate_config_value("theme", "dark").is_ok());

// Check models
let models_dir = std::path::Path::new("/path/to/models");
let missing = check_models_downloaded(&models_dir);

// Check file extension
assert!(is_media_file(std::path::Path::new("photo.jpg")));
```
