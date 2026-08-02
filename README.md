<img width="1584" height="672" alt="Siegu banner" src="https://github.com/user-attachments/assets/b0c940f9-2122-4bc6-a6b5-7d316430e3bb" />



[![License: FSL-1.1-Apache-2.0](https://img.shields.io/badge/License-FSL--1.1--Apache--2.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/built%20with-Tauri-blueviolet)](https://tauri.app/)
[![Vue.js](https://img.shields.io/badge/frontend-Vue.js%203-4fc08d)](https://vuejs.org/)
[![Rust](https://img.shields.io/badge/backend-Rust-000000)](https://www.rust-lang.org/)

**Siegu** (pronounced *see-goo*) is a privacy-first, local-only media management application. It organizes, secures, and synchronizes your photo and video library across devices without ever touching the cloud.

## Key Features

- **Local Semantic Search** — find photos by describing them ("sunset at the beach") using on-device CLIP models
- **Face Recognition** — automatic face detection and grouping via UltraFace + ArcFace
- **9 On-Device AI Models** — CLIP, face detection/recognition, OCR, NSFW, aesthetics, YOLO, BLIP, MiDaS, and Whisper, all running locally on ONNX Runtime
- **Peer-to-Peer Sync** — encrypted WebRTC sync between devices, no cloud required
- **Mesh Networking** — LAN discovery via mDNS, QR codes, or mnemonic phrases
- **Smart Library** — EXIF extraction, video indexing, heatmap, map view
- **Cross-Platform** — Linux, macOS, Windows, Android, iOS

## Quick Install

```bash
git clone https://github.com/denzyldick/siegu.git
cd siegu
npm install
npm run tauri dev
```

**Prerequisites**: Node.js v18+, Rust stable, system deps (see [docs/getting-started.md](docs/getting-started.md)).

## Documentation

For full documentation, see the `docs/` directory:

| Category | Docs |
|----------|------|
| **User** | [Getting Started](docs/getting-started.md), [Build](docs/build.md), [Configuration](docs/configuration.md), [CLI](docs/cli.md), [Sync Guide](docs/sync.md) |
| **Technical** | [Architecture](docs/architecture.md), [Database](docs/database.md), [ML Engine](docs/ml-engine.md), [Mesh Protocol](docs/mesh-protocol.md), [Frontend](docs/frontend.md), [Backend](docs/backend.md), [Android](docs/android.md), [iOS](docs/ios.md), [Developing](docs/developing.md), [Security](docs/security.md) |

## License

Licensed under **FSL-1.1-Apache-2.0** — commercial use allowed for non-competing products. Automatically becomes **Apache 2.0** on March 9, 2028.

See [LICENSE](LICENSE) for details.
