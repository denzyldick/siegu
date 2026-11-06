# Developing

## The contract-first loop

Siegu's rule: **Rust defines the command contract; frontends consume it.** So the
most important dev workflow is the one you use whenever you touch a command:

1. Edit the catalog: `crates/siegu-core/src/rpc_catalog.rs` → `spec(name, tier, stringify, args)`.
2. Regenerate the TS contract: `cargo build -p siegu-core` (build.rs rewrites
   `shared/generated/rpc-commands.ts`; keep it in the diff).
3. Implement the `dispatch` arm in `crates/siegu-core/src/rpc.rs`, putting shared
   logic in `library.rs` (or `ml_commands.rs`) and delegating from the Tauri
   wrappers.
4. Test at the facade (`rpc.rs::tests`), plus `ml_sec_tests` for boundary changes.
5. Confirm `generated_ts_matches_catalog` passes.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full contributor guide.

## Dev Setup

```bash
git clone https://github.com/denzyldick/siegu.git
cd siegu
npm install
cargo build            # builds workspace + regenerates the TS contract
npm run tauri dev
```

The `prepare` script automatically configures `core.hooksPath = .githooks` for pre-commit formatting.

## Githooks

Pre-commit hooks in `.githooks/pre-commit`:

1. **Rust formatting**: Runs `cargo fmt` on all workspace `.rs` files and re-stages changes
2. **JS/Vue formatting**: Runs `prettier --write` on staged `.js`, `.vue`, `.ts`, `.css`, `.json` files
3. **Translation check**: Runs `node scripts/check-translations.js` to verify all locale files have matching keys (English is canonical)

Enable hooks manually:

```bash
git config core.hooksPath .githooks
```

## CI

GitHub Actions workflows in `.github/workflows/` (details in `docs/ci.md`):

| Workflow                    | What it checks                                                                                                                                          |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ubuntu.yml`                | Unit/integration tests, lint, mesh + view-only E2E, face-grouping E2E, full AI inference (only platform that runs ML tests)                             |
| `macos.yml` / `windows.yml` | Unit tests, Tauri desktop build, mesh + view-only E2E                                                                                                   |
| `gpui.yml`                  | Locked `siegu-desktop` GPUI/Ely spike check + formatting on Linux, macOS, and Windows                                                                   |
| `android.yml`               | Cross-compile check (aarch64 + x86_64) + core tests on an x86_64 emulator                                                                               |
| `ios.yml`                   | Cross-compile check (aarch64) + core tests on an iOS simulator                                                                                          |
| `release.yml`               | Builds desktop installers + Android APK and gates on iOS build; triggered by a GitHub release or a `v*` tag push — artifacts auto-attach to the release |
| `signal-docker.yml`         | Docker publish (PRs only validate the build); mesh-sync E2E against the just-pushed image                                                               |

### Formatting

CI enforces:

- **Rust**: `cargo fmt --check` (Ubuntu only)
- **JS/Vue**: `bun x prettier --check` (Ubuntu only)
- **Translations**: `bun run check:translations` (Ubuntu only)

Run locally:

```bash
npm run format            # Fix formatting (Rust + frontend)
npm run format:check      # Check only
```

## Project Scripts

| Script                       | Description                        |
| ---------------------------- | ---------------------------------- |
| `npm run dev`                | Start Vite dev server              |
| `npm run build`              | Build frontend for production      |
| `npm run tauri dev`          | Start Tauri development mode       |
| `npm run tauri build`        | Build desktop app bundle           |
| `npm run test`               | Run vitest unit tests              |
| `npm run format`             | Format Rust + frontend code        |
| `npm run format:check`       | Check formatting without modifying |
| `npm run typecheck`          | Run vue-tsc type checking          |
| `npm run check:translations` | Verify locale completeness         |

## Rust Test Commands

```bash
# Core library tests (from the workspace root) — includes the catalog
# drift-guard (generated_ts_matches_catalog), ml_sec_tests, and rpc facade tests
cargo test -p siegu-core --lib

# Run a single rpc test module / test
cargo test -p siegu-core --lib rpc::tests::top_tags_and_location_names_report_seeded_library

# Integration tests (real mesh/sync transport)
cargo test -p siegu-core --test sync_e2e
cargo test -p siegu-core --test mesh_e2e

# Full peer-level E2E drivers (build the CLI first)
cargo build --release -p siegu-cli
scripts/e2e-view-only.sh     # view-only + sync guard + restore pull + RPC ro/rw ladder
scripts/e2e-sync.sh          # two-process mesh sync

# Tauri wrapper compile check only (heavy: full recompile)
cargo check -p siegu --manifest-path src-tauri/Cargo.toml

# Lint
cargo clippy -- -D warnings

# Formatting
cargo fmt --all -- --check
```

> The old "run `cargo test` from `src-tauri/`" habit is superseded: shared
> business logic now lives in `siegu-core`, so run `cargo test -p siegu-core`.

## GPUI + Ely version policy

The GPUI stack is pinned in the workspace manifest and moves as one unit:

- `gpui-pre = "=0.3.11"` (imported as `gpui`)
- `gpui-pre-platform = "=0.3.11"` (imported as `gpui_platform`)
- `ely-gpui-component` at git revision
  `3abe078a369a30b31638135a756f6a235e1f2982`

Rules:

1. **One GPUI version.** Never add `gpui-pre`, `gpui-pre-platform`, or
   `ely-gpui-component` directly to a crate manifest. Use the workspace
   dependencies in the root `Cargo.toml`. Mixing versions creates duplicate
   GPUI types that cannot cross crate boundaries.
2. **One Ely revision.** Every Ely crate comes from the same pinned revision.
   Do not mix a crates.io Ely release with the git pin.
3. **Upgrade together.** Bump `gpui-pre`, `gpui-pre-platform`, and the Ely git
   revision in one change. Regenerate `Cargo.lock`, check for duplicates, build
   the desktop spike, and re-run the Phase 0 benchmark before merging.
4. **Locked CI.** CI and release builds use the committed `Cargo.lock`
   (`cargo build --locked`). The lockfile records the exact Ely commit.
5. **Track by git, do not vendor (yet).** The git revision makes upgrades
   explicit and reproducible. Vendor Ely only if we need a local patch or the
   upstream history becomes unavailable; if we vendor, record the same revision
   in `Cargo.toml`.

Verify locally:

```bash
# Resolve without compiling and confirm the committed lockfile is current.
cargo metadata --locked --format-version 1 > /dev/null

# There must be exactly one line per GPUI package.
cargo tree -p siegu-ui --prefix none --format "{p}" \
  | grep -E '^gpui-pre(-platform)? v' \
  | cut -d' ' -f1-2 | sort -u

# Full compile check of the pinned UI crate.
cargo check -p siegu-ui --locked
```

## GPUI desktop spike

`crates/siegu-desktop` is the Phase 0 smoke test for the GPUI + Ely stack. It
opens a window with Ely `Heading`, `Paragraph`, and `Button` components and a
light/dark theme toggle.

### Linux packages

Ubuntu 24.04 (the list the Ely CI uses for the same stack):

```bash
sudo apt-get update
sudo apt-get install -y pkg-config libfontconfig-dev libxkbcommon-x11-dev
```

The spike enables both `gpui_platform` windowing backends (`wayland` and
`x11`). If your distribution does not resolve them transitively, install the
matching development packages too:

```bash
sudo apt-get install -y libwayland-dev libx11-dev libx11-xcb-dev libxkbcommon-dev
```

macOS needs the Xcode command-line tools (Metal); Windows needs the current
MSVC build tools. No additional system packages are required on either.

### Run and check

```bash
cargo run -p siegu-desktop
cargo check -p siegu-desktop --locked
cargo fmt -p siegu-desktop -- --check
```

CI runs the locked check and formatting on Linux, macOS, and Windows via
`.github/workflows/gpui.yml`.

## Neovim Debugging

The repo includes `.nvim.lua` with DAP configuration for debugging the Tauri backend with CodeLLDB:

```vim
:SieguTauriDev        " Start Tauri dev session
:SieguDebugAttach     " Attach CodeLLDB to the backend
```

Requires: `nvim-dap`, `codelldb` on PATH, Neovim with `exrc` enabled.

## Web host dev tips

```bash
# Serve a seeded library over HTTP (no ML by default)
siegu-cli web --port 8788 --config-path ./dev-scratch --share-mode rw

# Enable owner-tier ML for the web bearer (starts the live worker)
siegu-cli web --port 8788 --config-path ./dev-scratch --owner-mode

# The guest path pairs by code+token; use a --server signaler for cross-network
```

The browser SPA reaches the host through `src/services/backend/webHostBackend.ts`
(`fetch /rpc`), which is one of three `Backend` implementations
(`interface.ts`) selected at runtime by `src/services/runtime.ts`.

## Architecture

See `docs/architecture.md` for the workspace layout, the RPC facade, and the
tiered capability model — and `docs/e2e.md` for the test pyramid across modes.
