# Contributing to Siegu

Thanks for helping build Siegu. This guide is written **architecture-first**:
before touching code, understand the one rule that keeps the project coherent, and
the workflow it imposes. Then follow the setup, conventions, and PR process below.

---

## The one rule: Rust is the single source of truth

Siegu's frontends are peers, and **none of them own the command contract**.
The Rust core (`siegu-core`) defines every command the app can run, and every
platform frontend — desktop Tauri, the `siegu-cli web` host, and the WebRTC guest —
talks to **one** clean facade: `crates/siegu-core/src/rpc.rs::dispatch`.

```
        ┌────────────────────────────────────────────────────┐
        │                 siegu-core (Rust)                  │
        │  rpc_catalog.rs ──(source of truth)──► rpc::dispatch│
        │                                          │         │
        └──────────────────────────────────────────┼─────────┘
            ▲  ▲           ▲          ▲            │
            │  │           │          │            ▼
       desktop  webHost      guest      CLI      ML worker /
      (Tauri)  (browser)   (WebRTC)              database / mesh
```

- **Adding or changing a command?** Do it once, in Rust: add a `spec(...)` line to
  `crates/siegu-core/src/rpc_catalog.rs`, implement the arm in `rpc.rs::dispatch`,
  and put the shared business logic in `crates/siegu-core/src/library.rs` (or
  `ml_commands.rs`) so both RPC and the Tauri wrappers share it.
- **TypeScript is generated.** `crates/siegu-core/build.rs` parses the catalog and
  emits `shared/generated/rpc-commands.ts`. It is **committed**, and the Rust test
  `generated_ts_matches_catalog` fails CI if it ever drifts. Frontend engineers
  never hand-author the contract; they consume the generated types.
- **Never descend into ML/DB/worker internals from a frontend.** If a frontend
  needs data, there is (or should be) a command for it.

### Command tiers

Every command belongs to one of three tiers (see `Tier` in `rpc_catalog.rs`):

| Tier | Catalog guard | Who may call |
|------|---------------|--------------|
| `read` | `Tier::ReadOnly` | everyone (read-only and up) |
| `write` | `Tier::ReadWrite` | `rw` principals and above |
| `owner` | `Tier::Owner` | the **owner** only — bearer of the configured `web_token` at its own host `/rpc`, or the desktop user |

**Owner** is the trust boundary. Only owner-tier commands run ML analysis,
indexing, and device sync. A WebRTC guest (code + token on `siegu.io` or a
remote signaler) is **capped at `write`** and can never become owner, no matter
what it sends. `siegu-cli web` opens ML to the web bearer only when launched with
`--owner-mode`; without it the bearer is capped at `--share-mode` (`ro`/`rw`).

See [docs/architecture.md](docs/architecture.md#rpc-facade) for the detail.

---

## The contract-first workflow

Any change that touches commands follows this order:

1. **Edit the catalog** — `rpc_catalog.rs`: add/change the `spec(name, tier, stringify, args)`.
2. **Regenerate the TS contract** — `cargo build -p siegu-core` (build.rs rewrites
   `shared/generated/rpc-commands.ts`; keep the regenerated file in the diff).
3. **Implement the dispatch arm** in `rpc.rs` (respecting the catalog's tier via
   the allowlist/gates), putting shared logic in `library.rs` / `ml_commands.rs`.
4. **Wire the Tauri wrapper** to delegate to the shared helper (don't duplicate).
5. **Test at the facade** — add/update a test in `rpc.rs::tests` that calls
   `dispatch` and asserts behavior, plus the tier-security tests
   (`ml_sec_tests`) when capability boundaries change.
6. **Confirm the drift guard** — `generated_ts_matches_catalog` must pass.

If you change a tier or add/remove/rename a command, run `cargo test -p siegu-core`
and confirm `shared/generated/rpc-commands.ts` is updated in the same commit.

---

## Setup

```bash
git clone https://github.com/denzyldick/siegu.git
cd siegu
bun install          # or: npm install
cargo build          # builds the workspace, regenerates the TS contract
bun run tauri dev    # desktop app
```

You'll need [Node.js](https://nodejs.org) 20.19+, [Bun](https://bun.sh) (or npm),
and [Rust](https://rustup.rs/). Platform-specific dependencies live in
[docs/getting-started.md](docs/getting-started.md). The `prepare` script enables
the pre-commit hooks automatically.

---

## Conventions

- **Rust** — Rust 2021 edition, `cargo fmt` style, no `unsafe` unless
  unavoidable and documented. Put shared, testable business logic in
  `siegu-core`; keep `src-tauri` thin wrappers.
- **TypeScript/Vue** — Vue 3 `<script setup lang="ts">`, Vuetify 3, Pinia.
  Prettier formatting. Frontends consume the generated contract, never define
  command names inline.
- **Command naming** — lowercase `snake_case` (e.g. `get_top_tags`,
  `toggle_favorite`). The generated contract and the casing helper
  `src/services/backend/rpcCasing.ts` assume snake_case.
- **Commits** — conventional commits, matching the repo history:
  `refactor(#42):`, `feat(#19):`, `fix(#42):`, `test(ml):`, `docs:`. Reference
  the issue number.
- **No secrets** — never commit tokens, keys, or the demo `web_token`.

---

## Hooks, lint, and CI

Pre-commit hooks (`.githooks/pre-commit`) run:

1. **Rust formatting** — `cargo fmt` on workspace `.rs` files (re-stages).
2. **JS/Vue formatting** — `prettier --write` on staged web files.
3. **Translation check** — `node scripts/check-translations.js` (`en` is the
   canonical key set; other locales must match).

CI (`.github/workflows/`) enforces `cargo fmt --check`, `prettier`, the
translation check, `cargo test`, the mesh/sync E2Es, and (on releases)
desktop/mobile builds. See [docs/ci.md](docs/ci.md).

### Local verification (quick)

```bash
cargo test -p siegu-core          # core tests incl. the drift guard
cargo fmt --all -- --check        # formatting (as the hook does)
npm run typecheck && npm run test  # frontend types + vitest
npm run check:translations        # locale completeness
```

Full E2E guidance is in [docs/e2e.md](docs/e2e.md): desktop Tauri, the `siegu-cli web`
host (`--owner-mode` for ML parity), and the WebRTC guest round-trip.

---

## Where things live

| Concern | Location |
|---------|----------|
| Command catalog (source of truth) | `crates/siegu-core/src/rpc_catalog.rs` |
| RPC facade (`dispatch`) | `crates/siegu-core/src/rpc.rs` |
| Generated TS contract (committed) | `shared/generated/rpc-commands.ts` |
| Shared RPC/desktop business logic | `crates/siegu-core/src/library.rs`, `ml_commands.rs` |
| Capability + auth | `ShareMode` / `Tier`; `siegu-cli web --share-mode`, `--owner-mode` |
| Frontend  Backend seam | `src/services/backend/{interface,tauriBackend,webHostBackend,guest}.ts` |
| Tauri command wrappers | `src-tauri/src/commands/*.rs` |
| Web host server | `crates/siegu-cli/src/web.rs` |

See [docs/architecture.md](docs/architecture.md) for the full map.

## Submitting a PR

- Keep the contract, its generated artifact, and the implementing frontends in
  the same change set so nothing drifts.
- Include or reference tests: facade tests for new commands, and tier-security
  tests for any capability-boundary change.
- Run the quick verification above before opening the PR.
- The GitHub Actions suite runs on every PR; a green CI is required to merge.
