# Developing

## Dev Setup

```bash
git clone https://github.com/denzyldick/siegu.git
cd siegu
npm install
npm run tauri dev
```

The `prepare` script automatically configures `core.hooksPath = .githooks` for pre-commit formatting.

## Githooks

Pre-commit hooks in `.githooks/pre-commit`:

1. **Rust formatting**: Runs `cargo fmt` on all workspace `.rs` files and re-stages changes
2. **JS/Vue formatting**: Runs `prettier --write` on staged `.js`, `.vue`, `.ts`, `.css`, `.json` files
3. **Translation check**: Runs `node scripts/check-translations.js` to verify all locale files have matching keys

Enable hooks manually:
```bash
git config core.hooksPath .githooks
```

## CI

GitHub Actions workflows in `.github/workflows/` (details in `docs/ci.md`):

| Workflow | Jobs |
|----------|------|
| `ci.yml` | `test` (macOS/Ubuntu/Windows: fmt, check, test, clippy, tauri build), `test-android`, `test-ios`, ML inference E2E, release builds |
| `e2e.yml` | `face-grouping` (ML), `sync-cli` (3-OS mesh sync) |
| `signal-docker.yml` / `landing-page-docker.yml` | Docker publish + build-only PR validation; `signal-docker.yml` also runs the mesh-sync E2E against the just-pushed commit image |

### Formatting

CI enforces:
- **Rust**: `cargo fmt --check` (in `src-tauri/`)
- **JS/Vue**: `npm run format:check` (prettier)
- **Translations**: `npm run check:translations`

Run locally:
```bash
npm run format    # Fix formatting
npm run format:check  # Check only
```

## Neovim Debugging

The repo includes `.nvim.lua` with DAP configuration for debugging the Tauri backend with CodeLLDB:

```vim
:SieguTauriDev        " Start Tauri dev session
:SieguDebugAttach     " Attach CodeLLDB to the backend
```

Requires: `nvim-dap`, `codelldb` on PATH, Neovim with `exrc` enabled.

## Project Scripts

| Script | Description |
|--------|-------------|
| `npm run dev` | Start Vite dev server |
| `npm run build` | Build frontend for production |
| `npm run tauri dev` | Start Tauri development mode |
| `npm run tauri build` | Build desktop app bundle |
| `npm run test` | Run vitest unit tests |
| `npm run format` | Format Rust + frontend code |
| `npm run format:check` | Check formatting without modifying |
| `npm run typecheck` | Run vue-tsc type checking |
| `npm run check:translations` | Verify locale completeness |

## Rust Test Commands

```bash
# Run all tests (from src-tauri/)
cargo test

# Run core library tests
cargo test -p siegu-core

# Run ignored (integration) tests
cargo test -- --ignored test_full_inference_on_sample
cargo test -- --ignored test_whisper_smoke

# Run the mesh sync E2E (Rust-level, no models needed)
cargo test -p siegu-core --test sync_e2e
cargo test -p siegu-core --test mesh_e2e    # join --initiator, delta sync, mDNS

# Run the CLI-level sync E2E (builds + exercises two real processes)
bash scripts/e2e-sync.sh

# Run lint
cargo clippy -- -D warnings
```

## Architecture

See `docs/architecture.md` for workspace layout and module documentation.
