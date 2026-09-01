# Architecture

## The shape of the system

Siegu has **one source of truth and several peers.** `siegu-core` (Rust) owns the
command contract and every frontend — desktop Tauri, the `siegu-cli web` host, and the
WebRTC guest — sits above a single clean facade (`rpc::dispatch`). Rust defines
the contract; TypeScript is generated from it.

```
        ┌─────────────────────────────────────────────────────────┐
        │                     siegu-core (Rust)                    │
        │  rpc_catalog.rs ──► shared/generated/rpc-commands.ts     │
        │  (source of truth)        (committed, drift-guarded)     │
        │                                   │                      │
        │                                   ▼                      │
        │                         rpc.rs::dispatch (the facade)    │
        │                          Tier │ allowlists │ gates       │
        │                            │                            │
        │          ┌─────────────────┼──────────────────┐          │
        │          │                 │                  │          │
        │   library.rs          ml_commands.rs      database /
        │   (shared logic)       (ML, owner-only)     mesh / worker │
        └──────────┬─────────────────┬──────────────────┬──────────┘
                   ▲                 ▲                  ▲
            desktop (Tauri)    webHost (browser)   guest (WebRTC)
```

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

## The RPC facade

### Command catalog (source of truth)

`crates/siegu-core/src/rpc_catalog.rs` defines every command — 70 today — with:

```rust
spec(name, Tier, stringify, &[args])
```

- **`Tier::ReadOnly`** (34 commands) — read-only, available to every principal.
- **`Tier::ReadWrite`** (26 commands) — mutations (favorites, trash, albums),
  available to `rw` principals and above.
- **`Tier::Owner`** (10 commands) — ML analysis/indexing and heavy host work
  (`analyze_photo`, `analyze_model`, `index_faces`, `pause/resume/abort_indexing`,
  `reload/unload_models`, `get_models_loaded`, ...). **Owner only.**

`rpc.rs::dispatch(ctx, name, payload)` uses the catalog to drive its allowlist and
tier gates, so a command can't be silently added without a tier or bypass the
owner boundary.

### Generated TypeScript contract

`crates/siegu-core/build.rs` parses the catalog and emits
`shared/generated/rpc-commands.ts` (interface + `RPC_COMMANDS` array with tier and
arg names). The file is **committed** and guarded by the Rust test
`generated_ts_matches_catalog`, which fails if the committed TS ever drifts from
the catalog. Frontends import these generated types instead of hand-authoring
command names/casing. Regenerate with `cargo build -p siegu-core`.

### Capability model

| Principal | Capability | ML |
|-----------|-----------|-----|
| Desktop user | everything | yes |
| **Owner** — bearer of the configured `web_token` at its own host `/rpc` | everything | yes (opt-in) |
| `rw` guest (WebRTC/mesh, or web without `--owner-mode`) | read + write | no |
| `ro` guest | read only | no |

- `siegu-cli web --share-mode ro|rw` caps web/WebRTC/mesh guests. Default `ro`.
- `siegu-cli web --owner-mode` promotes the bearer of the printed `web_token` to
  `ShareMode::Owner` at that host and starts the live ML worker. It implies
  `rw`, which then applies only to guests. Without it the web bearer stays
  capped by `--share-mode` and `ml: None`.
- There are **no accounts**; `Owner` is defined solely as the token holder at
  their own host's `/rpc`. A remote guest can never reach owner tier.

### Shared business logic

Logic used by both the RPC facade and the Tauri command wrappers lives in
`library.rs` (non-gated) and `ml_commands.rs` (`ml`-gated). The Tauri
`commands/*.rs` wrappers delegate to these helpers, so there is **one**
implementation of e.g. `get_top_tags`, `get_location_names`, `delete_face`, or
`get_unindexed_count` instead of twin copies.

## Workspace Layout

```
siegu/
├── crates/siegu-core/            # Core library — no Tauri dependency
│   ├── build.rs                  # Generates shared/generated/rpc-commands.ts
│   └── src/
│       ├── rpc.rs                # dispatch() — the one RPC facade
│       ├── rpc_catalog.rs        # single source of truth for commands
│       ├── library.rs            # shared RPC/desktop business logic
│       ├── ml_commands.rs        # owner-tier ML commands (ml-gated)
│       ├── database.rs           # SQLite schema + queries
│       ├── config.rs             # config validation
│       ├── error.rs              # SieguError
│       ├── event_bus.rs          # EventBus trait
│       ├── face_detector.rs      # UltraFace anchor math + NMS
│       ├── geocode.rs            # offline reverse geocoding
│       ├── lan_server.rs         # Warp WebSocket signaling (+ pairing)
│       ├── mdns.rs               # mDNS service registration
│       ├── mesh.rs               # Sync: Manifest, FileChunk, RPC over transport
│       ├── mesh_transport.rs     # WebRTC + WebSocket transport
│       ├── ml_engine/            # ML inference pipeline
│       ├── ml_worker.rs          # ML job queue, MlContext
│       ├── model_manager.rs      # model registry
│       ├── scanner.rs            # EXIF extraction, ScanGuard
│       ├── server.rs             # BIP39 pairing codes
│       ├── shutdown.rs           # ShutdownCoordinator
│       ├── signal.rs / signalling.rs  # SignalMessage, signaling URL handling
│       ├── sync_transport.rs     # filename sanitization
│       ├── thumbnail.rs          # 320px base64 JPEG thumbnails
│       ├── view_only.rs          # view-only/album-share mesh scoping
│       └── logfmt.rs             # log formatting
│
├── crates/siegu-cli/             # CLI binary (clap)
│   └── src/
│       ├── main.rs               # command groups: scan/analyze/mesh/web/...
│       └── web.rs                # `siegu-cli web` host: static SPA + /rpc → dispatch
│
├── src-tauri/                    # Tauri desktop/mobile shell
│   └── src/
│       ├── lib.rs, main.rs       # setup, plugins, commands
│       ├── commands/             # thin wrappers delegating to siegu-core
│       ├── ml.rs, transport.rs, thumbnail.rs, file.rs, notify.rs
│       ├── tauri_sync_event.rs   # SyncEvent → Tauri events bridge
│       └── ... (mdns_plugin, permission_plugin, wallpaper_plugin, ...)
│
├── src/                          # Vue 3 frontend (one bundle, three modes)
│   ├── services/backend/         # interface.ts + tauri/webHost/guest backends
│   │   └── rpcCasing.ts          # snake_casing driven by the generated contract
│   ├── stores/, composables/, components/, types/, locales/
│   └── main.ts                   # runtime mode detection (desktop/webHost/guest)
│
├── webclient/                    # standalone guest demo (superseded by the src bundle)
├── shared/generated/             # committed generated TS contract
├── scripts/                      # e2e drivers, translation check, android build
└── .github/workflows/            # CI (per-platform tests + E2E, releases, docker)
```

## Runtime modes

One Vue bundle serves three modes, chosen at runtime (`src/services/runtime.ts`):

| Mode | Trigger | Backend |
|------|---------|---------|
| `desktop` | Tauri (`isTauri`) | `tauriBackend` (invoke) |
| `webHost` | not Tauri + `GET /session` returns a code | `webHostBackend` (fetch `/rpc`) |
| `guest` | not Tauri + `#code.token` in the hash | `guest` (WebRTC RPC) |

All three implement the same `Backend` interface, so stores/views never branch on
share mode — authorization is entirely server-side in `dispatch`.

## Feature Flags (siegu-core)

| Feature | Deps | Purpose |
|---------|------|---------|
| `ml` (default) | `ort`, `tokenizers` | AI model inference |
| `video-thumbs` | `ffmpeg-next` | Video keyframe extraction |

`ml` gates the owner-tier ML commands and the live worker; without it the facade
still answers (ML arms return "needs a live ML worker" for an owner without a
worker, and are blocked for everyone else).

## E2E & contract tests

- `generated_ts_matches_catalog` pins the committed TS to the Rust catalog.
- `ml_sec_tests` pins the capability boundary (guests blocked from owner-tier ML;
  owner-with-worker allowed).
- RPC contract-freeze tests cover uncapped `get_unindexed_count` and populated
  `get_top_tags` / `get_location_names` through `dispatch`.
- Shell E2Es (`scripts/e2e-view-only.sh`, `e2e-sync.sh`) drive real processes over
  WebRTC, incl. the ro/rw RPC reject/allow ladder.

See [e2e.md](e2e.md), [backend.md](backend.md), and [frontend.md](frontend.md).