# End-to-End Testing

Siegu's contract lives in Rust and is drift-guarded, so **E2E tests are layered**:
fast Rust unit tests that pin the facade, shell exercises that drive real CLI
processes over real WebRTC/mesh transport, and a manual web-host flow that
verifies the browser data plane (including `--owner-mode` ML parity).

Everything below assumes you built the CLI: `cargo build --release -p siegu-cli`
(or use `target/debug/siegu` for faster iteration).

---

## 1. Unit + facade tests (fast, no models)

Run the whole core suite — this includes the contract drift guard
(`generated_ts_matches_catalog`), the ML capability-boundary tests
(`ml_sec_tests`), and the RPC contract-freeze tests:

```bash
cargo test -p siegu-core --lib
```

Key things these pin:
- **Contract drift**: the committed `shared/generated/rpc-commands.ts` matches
  `rpc_catalog.rs`. If you changed a command, regenerate with
  `cargo build -p siegu-core` and re-run.
- **Capability ladder**: a guest (read-only / `rw`, no worker) is *rejected* from
  every owner-tier ML command; the owner (with a live worker) is allowed.
- **Facade behavior**: `dispatch` returns real data over the RPC surface — e.g.
  `get_unindexed_count` reports the uncapped library count (>50), and
  `get_top_tags` / `get_location_names` surface seeded tags and locations.

Tauri wrapper tests (previously separate) now mostly delegate to the shared Rust
helpers, so the important behavior is covered by the core suite. The Tauri crate
still compiles standalone: `cargo check -p siegu --manifest-path src-tauri/Cargo.toml`.

---

## 2. CLI "contract" level (no UI, no browser)

The `siegu web` / `siegu mesh` commands exercise the **same `dispatch` facade**
the frontends use. Verify your flags and share-mode mapping:

```bash
# web host command surface + help
./target/debug/siegu web --help

# quick RPC sanity against a tiny library
./target/debug/siegu --config-dir /tmp/demo scan tests/fixtures
./target/debug/siegu --config-dir /tmp/demo web --port 8788 --share-mode rw &
```

---

## 3. Mesh / sync E2E (real processes, no models)

These drive two real siegu processes over the actual WebRTC/mesh and signaling
stack.

### Rust integration tests (single crate, deterministic)
```bash
cargo test -p siegu-core --test sync_e2e     # delta sync, mDNS, sync over transport
cargo test -p siegu-core --test mesh_e2e     # join --initiator + session
cargo test -p siegu-core --test signal_routing
```

### Shell drivers (full CLI + WebRTC)
- **`scripts/e2e-sync.sh`** — builds and runs two real processes; verifies
  initiator/joiner mesh sync end to end.
- **`scripts/e2e-view-only.sh`** — the richest contract exercise. It hosts a
  real WebRTC session and validates, by greppable markers:
  1. **View-only entry** — chunked manifest + thumbnail via the view-only cache.
  2. **Sync guard** — the sharer ignores `StartSync` from a view-only peer.
  3. **Restore pull** — one original re-materializes byte-for-byte (SHA-256).
  4. **RPC over WebRTC** — `list_files` works under `ro`; `toggle_favorite` is
     **rejected** under `ro` and **succeeds** under `rw`.
  5. **Album share links** — a member gets a scoped single-photo manifest while
     a non-member is denied; two guests served concurrently.

```bash
scripts/e2e-view-only.sh         # env: SIEGU_BIN, SIEGU_E2E_PHOTOS
scripts/e2e-sync.sh
```

These run in the `mesh-e2e` / `ai-inference` CI jobs (see `docs/ci.md`).

---

## 4. Browser / web host data plane (manual)

The web host serves the browser build and its `/rpc` endpoint bridges to the same
`dispatch` facade. This is where you verify the **web host ↔ guest** model and the
opt-in ML parity:

```bash
# 1. Build the browser bundle you serve
bun install && bun run build

# 2. Serve a scanned library from the web host, default (no ML, ro)
./target/debug/siegu web --port 8788 --config /tmp/demo --share-mode rw

# 3. Open http://localhost:8787 and browse/search — the browser drives /rpc.
#    (The SPA talks to the host over HTTP via src/services/backend/webHostBackend.ts.)

# 4. ML parity: relaunch with the owner flag
./target/debug/siegu web --port 8788 --config /tmp/demo --owner-mode
#    Now the bearer of the printed `web_token` at this host's /rpc is Owner:
#    analyze_photo / index_faces / analyze_model become available, the live ML
#    worker runs, and --share-mode still caps any WebRTC/mesh guests.
```

### What to verify manually
- **Ro vs rw**: with `--share-mode ro`, write commands from the web bearer are
  rejected; with `rw` they apply. Same mapping the E2E driver asserts over
  WebRTC.
- **Owner gating**: without `--owner-mode`, ML commands fail for the web bearer
  even under `rw`; with `--owner-mode` they run.
- **Casing**: guest/WebRTC RPC payloads use snake_case keys
  (`src/services/backend/rpcCasing.ts` agrees with the generated contract).
- **Frontend unit tests** (fast, no host): `npm run test` covers the Backend
  seam, incl. snake-case payload tests for `guest.ts` and `webHostBackend.ts`.

---

## CI

CI runs the unit + integration suites and the shell E2Es per platform
(`docs/ci.md`). The `mesh-e2e` jobs execute `e2e-view-only.sh` (and sync), so a
green CI is the strongest contract guarantee short of the browser-facing manual
flow above.
