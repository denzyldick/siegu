# CI / E2E Testing

All CI runs on GitHub Actions (`.github/workflows/`). This page documents each
workflow, the model downloads, the E2E scripts, and how to run everything
locally.

## Workflows

CI is organized **per platform**: each supported OS has its own workflow whose
jobs cover every check that platform can run (unit tests + lint, the mesh-sync
E2E, and the real-ML inference test on desktop). Open the workflow for a
platform in the Actions tab to see its full check list, then switch to another
platform's list.

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ubuntu.yml` | push to `main`, PRs | Ubuntu: `tests`, `mesh-e2e`, `ai-inference` jobs |
| `macos.yml` | push to `main`, PRs | macOS: `tests`, `mesh-e2e`, `ai-inference` jobs |
| `windows.yml` | push to `main`, PRs | Windows: `tests`, `mesh-e2e`, `ai-inference` jobs |
| `android.yml` | push to `main`, PRs | Android: cross-compile check + core tests on an arm64 emulator |
| `ios.yml` | push to `main`, PRs | iOS: cross-compile check + core tests on a simulator |
| `release.yml` | tags, releases | Build/publish desktop installers, Android APK, Arch AppImage, iOS |
| `signal-docker.yml` | push to `main`, PRs, tags | Build/push the signaling-server Docker image; PRs only validate the build |
| `landing-page-docker.yml` | push to `main`, PRs, tags | Build/push the landing-page Docker image; PRs only validate the build |

### Desktop workflows (`ubuntu.yml`, `macos.yml`, `windows.yml`)

Each runs three jobs on its platform:

- **`tests`**: `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
  (which subsumes `cargo check`), a second clippy run gating
  `-D clippy::unwrap_used -D clippy::expect_used` on production code (lib + bins;
  tests are excluded so they may keep unwraps), and
  `npm run tauri build -- --no-bundle`. The unwrap/expect gate also runs in the
  pre-commit hook, so no new panics-on-error can be added to shipped code.
  macOS builds ONNX Runtime with the CoreML feature; other platforms use plain
  or DirectML (see `crates/siegu-core/Cargo.toml` `[target.*.dependencies]`).
- **`mesh-e2e`**: builds `siegu-cli` and runs `scripts/e2e-sync.sh`. Two CLI
  processes connect over WebRTC through an in-process signaling server, exchange
  protocol messages, and transfer a photo byte-for-byte (SHA-256 compared). No ML
  models required. On Ubuntu the same CLI build also runs
  `scripts/e2e-face-grouping.sh` (one release build instead of two), which
  downloads face-detection models to `/tmp/siegu-e2e-models` (cache key
  `siegu-e2e-models-v2`) and asserts that same-person photos are grouped into one
  album by the AI pipeline.
- **`ai-inference`**: downloads the model suite to `src-tauri/test_models/`
  (cache key `ai-test-models-v3-${{ runner.os }}`) and runs the two `#[ignore]`d
  integration tests in `src-tauri/src/ml.rs`: `test_full_inference_on_sample`
  and `test_whisper_smoke`.

### Mobile workflows

- **`android.yml`** — `cargo ndk check` for `aarch64-linux-android`, builds the
  `siegu-core` unit + `sync_e2e` test harnesses for the same target, and runs
  them on an arm64 Android emulator (ort only ships arm64 Android binaries).
- **`ios.yml`** — `cargo check` for `aarch64-apple-ios`, builds the same
  harnesses for `aarch64-apple-ios-sim`, and runs them on an iOS simulator.

### Docker publish workflows

- `signal-docker.yml` — image `ghcr.io/denzyldick/siegu-signal`, built from
  `crates/siegu-signal/Dockerfile` (repository root is the build context).
  Container reads `PORT` (default `8080`) and optional `SIEGU_SIGNAL_TOKEN`,
  exposes a `/healthz` endpoint, and runs as a non-root user. After publishing,
  a `mesh-sync-e2e` job runs `scripts/e2e-sync.sh` against the
  commit-sha-tagged container just pushed (not `latest`), so the exact image
  built from the current commit is exercised.
- `landing-page-docker.yml` — image `ghcr.io/denzyldick/siegu-landing-page`,
  built from `landing-page/Dockerfile` (`node:20-alpine`, `npm ci --omit=dev`).
- Both add a build-only `build-image` job on PRs so image breakage is caught
  before merge without publishing.
- Tagging: pushes to `main` get `main` + `<commit-sha>`; version tags get
  `semver` tags + `<commit-sha>` and set `latest` (so `latest` always points at
  the newest release, never at unreleased `main`).

## Platform coverage

What is actually verified per platform, and on which devices the app can run.

| Coverage | macOS | Ubuntu | Windows | Android | iOS |
|----------|-------|--------|---------|---------|-----|
| Unit/integration tests + lint | ✅ | ✅ | ✅ | — | — |
| Real AI inference (full model suite) | ✅ | ✅ | ✅ | — | — |
| Mesh sync E2E (CLI, in-process signaling) | ✅ | ✅ | ✅ | — | — |
| ML face-grouping E2E | — | ✅ | — | — | — |
| Container mesh E2E | — | ✅ | — | — | — |
| Cross-compile check | — | — | — | ✅ | ✅ |
| On-device unit tests + mesh sync (`sync_e2e`) | — | — | — | ✅ emulator | ✅ simulator |
| Release artifacts | ✅ | ✅ | ✅ | ✅ APK | — |

**Policy**: real AI *inference* is verified on desktop only; mobile guarantees
compile-time support. The shared runtime (DB, WebRTC mesh) is exercised
on-device via the Android emulator and iOS simulator jobs at least once — these
may be disabled if they prove too slow in CI.

**Supported devices** (determined by which prebuilt ONNX Runtime binaries exist
for `ort`'s `download-binaries` strategy, plus per-OS EP features):

- Windows x86_64 (Win 10/11) — DirectML GPU acceleration
- Windows 11 on ARM (e.g. Snapdragon X) — CPU
- macOS on Apple Silicon (M1–M4) — CoreML / Neural Engine
- Linux x86_64 (glibc distros) — CPU
- Linux ARM64 (Raspberry Pi 4/5, ARM SBCs) — CPU
- Android arm64 (every modern phone/tablet, ~2017+) — CPU
- iPhone/iPad (arm64; iOS 12+) — CoreML
- iOS Simulator on Apple Silicon (development)

Not supported: Intel Macs, 32-bit platforms, Android x86_64 (emulator images
must be arm64), Linux musl/Alpine.

## E2E scripts

| Script | What it does |
|--------|--------------|
| `scripts/e2e-sync.sh` | Builds the CLI, starts a mesh host, joins from a second process, transfers `einstein_1.jpg`, asserts the transferred file matches the source SHA-256. Falls back to the external signaling server when `SIEGU_SIGNAL_URL` is set. |
| `scripts/e2e-face-grouping.sh` | Runs the full AI pipeline against a small album and asserts same-person faces land in one group. Needs the model suite in `SIEGU_MODELS_DIR` (default: downloads to the script's own cache). |

### Running the sync E2E locally

```bash
# LAN mode (in-process signaling server)
bash scripts/e2e-sync.sh

# External signaling server
docker run --rm -p 8080:8080 ghcr.io/denzyldick/siegu-signal:latest
SIEGU_SIGNAL_URL=ws://127.0.0.1:8080 bash scripts/e2e-sync.sh

# Or: a bare `siegu serve` on one terminal, then the script with SIEGU_SIGNAL_URL
```

The Rust-level equivalents live in `crates/siegu-core/tests/` and run with
`cargo test -p siegu-core` (no models needed):

- `sync_e2e.rs` — two peers exchange protocol messages over a LAN signaling
  server (also honors `SIEGU_SIGNAL_URL` for external servers).
- `mesh_e2e.rs` — three scenarios:
  - `two_joiners_connect_with_initiator_flag`: the `mesh join --initiator`
    path, where two joiner peers (neither is the LAN host) connect.
  - `mesh_delta_sync_transfers_only_new_photos`: reconnect after a peer adds
    a photo and assert only the new file transfers (delta sync).
  - `mdns_discovers_lan_host`: registers the `_siegu._tcp` service and
    verifies LAN discovery finds it (skips only if mDNS is unavailable).

### Running the ML E2E locally

The inference tests need the full model suite (~5 GB) in `src-tauri/test_models/`.
CI downloads it automatically; locally the tests fail fast with a skip message
if any file is missing. To force-run them:

```bash
cd src-tauri
cargo test -- --ignored test_full_inference_on_sample
cargo test -- --ignored test_whisper_smoke
```

`test_full_inference_on_sample` analyzes `tests/fixtures/faces/einstein_1.jpg`,
runs face detection/recognition, captioning (BLIP), OCR, aesthetics/NSFW
scoring and MiDaS depth, then asserts a coherent caption and ≥1 detected face.

## Model suite (AI inference job)

All files land in `src-tauri/test_models/`:

| File | Source |
|------|--------|
| `clip-vit-base-patch32-visual.onnx` / `-text.onnx`, `tokenizer.json` | `Xenova/clip-vit-base-patch32` |
| `face_detection_yunet_2023mar.onnx` | opencv_zoo |
| `ocr_det.onnx`, `ocr_rec.onnx`, `en_dict.txt` | SWHL RapidOCR / PaddleOCR |
| `nsfw.onnx` | onnx-community nsfw_image_detection |
| `aesthetics.onnx` | aesthetic-predictor-v2-5 |
| `yolov8.onnx` | webml/yolov8n |
| `blip.onnx` (split_0), `blip_decoder.onnx` (split_1), `blip_tokenizer.json` | Salesforce BLIP image-captioning-base |
| `arcface.onnx` | arcface_w600k_r50 |
| `midas.onnx` | Xenova dpt-hybrid-midas |
| `whisper.onnx` (encoder), `whisper-decoder.onnx`, `whisper-tokenizer.json` | onnx-community whisper-tiny |

Cache key is `ai-test-models-v3` — bump it whenever a model URL or file
changes so CI re-downloads instead of reusing stale files.

## Known issues

- `crates/siegu-signal/Cargo.toml` declares `siegu-core` with
  `default-features = false`, but because the workspace dependency
  (`Cargo.toml`) does not pin `default-features`, Cargo **ignores** that and
  the signaling container compiles the full ML stack anyway (slow builds, C++
  runtime deps). Fixing it properly requires `#[cfg(feature = "ml")]` gating
  across `siegu-core`'s lib code first — the crate currently references
  `ort`/`tokenizers` unconditionally, so the signal binary won't build without
  the `ml` feature. See the "default-features is ignored for siegu-core"
  warning from `cargo build`.

- The signaling Docker image pins the builder to `rust:1-slim-bookworm` and
  the runtime to `debian:bookworm-slim`. Both must stay on the same major
  Debian release: the floating `rust:1-slim` base has moved to Debian 13,
  which produced binaries that fail to load on the bookworm runtime
  (`GLIBC_2.38` / `GLIBCXX_3.4.31` missing). When either base is eventually
  bumped, bump both stages together. (The tag is `-slim-bookworm`, not
  `-bookworm-slim`.)
