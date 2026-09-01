# Web Runtime Modes: Hosted Web Instance + web.whatsapp.com-style Guest

## Purpose

One `siegu-cli`/desktop binary / one Vue build serves the UI in three runtime modes:

- **`desktop`** — the native Tauri app (unchanged, existing behavior).
- **`webHost` (Mode A)** — the browser is the **owner** of a library mounted on a
  VPS / `siegu-cli web` instance (e.g. `docker compose` with `/data` mounted). Full
  GUI over HTTP; no pairing.
- **`guest` (Mode B)** — a browser on another device **pairs by code + token** with
  a remote or desktop Siegu over an Internet `wss://` signaler, and **streams
  pictures** over WebRTC — the web.whatsapp.com model.

The web build is the same artifact; the mode is chosen at runtime.

---

## Why this exists (conflict with a prior mDNS guard)

`discoverLanDevices` / `pingMdnsPlugin` call Tauri's `invoke()`, which is
undefined in a plain browser. An earlier guard (`isTauri` in
`src/services/tauri.ts`) stops the crash, but it only hides the connect screen —
it does **not** give Mode B a working discovery/pairing path. Mode B genuinely
needs a browser-native pairing mechanism, which this plan provides.

---

## Current state (facts from a code pass)

- `siegu-cli web` today is already a **pairing host**: it embeds a loopback signaling
  server, creates a room `code`, and prints `http://HOST:PORT/#code.token`
  (`crates/siegu-cli/src/web.rs`). It serves the **desktop-only** Vue bundle.
- The `#19` Backend seam exists but is **unused by stores/components**:
  - `src/services/backend/{interface,createBackend,tauriBackend,guestBackend,guest,peer,protocol}.ts`
  - Stores call `@/services/tauri` directly; media components call
    `useMediaUrl()` → `127.0.0.1` URLs (break in a browser).
- **No runtime mode detection** exists in `src/main.ts` / `App.vue`.
  `isTauri` (`src/services/tauri.ts:382`) is the only browser-vs-desktop signal.
- WebRTC guest round-trip is **fully implemented on host + client**: Rust in
  `crates/siegu-core/src/mesh.rs` (`EnterViewOnly`/`EnterAlbumShare`/
  `FetchMediaRequest`/`CommandRequest`) and browser in `guest.ts`/`peer.ts`/
  `protocol.ts`. Proven end-to-end in `webclient/src/main.ts`.
- Internet/hosted signaling is **designed-for but unused**: `wss://` default in
  `normalize_signaling_url` (`crates/siegu-core/src/signalling.rs:19`), default
  `wss://siegu.io/ws` (`src/services/signalling.ts:4`), code/token pairing in
  `crates/siegu-core/src/lan_server.rs:494-607`.

---

## Mode detection (`src/services/runtime.ts`)

| Mode | Trigger (in order) |
|------|--------------------|
| `desktop` | `isTauri === true` |
| `webHost` | not Tauri **and** `fetch('/session')` returns `{code}` (`web.rs:153-155`) |
| `guest` | not Tauri, no `/session`, **and** `parseHash(location.hash)` finds `#code.token` (`protocol.ts:41`) |
| `onboarding` | none of the above (fresh / landing) |

`createBackend.ts` extends `BackendMode` to `'tauri' | 'webHost' | 'guest'`.

---

## Mode A data path decision (recommended: **Option 1**)

The browser in Mode A is the **owner** of its local library; it should not pair
with itself over WebRTC.

- **Option 1 — WebHost over HTTP/RPC (RECOMMENDED).** New Rust HTTP-RPC routes on
  `siegu-cli web` mirroring `rpc::dispatch` (`crates/siegu-core/src/rpc.rs:161-261`),
  plus HTTP media routes; new `webHostBackend.ts` `fetch`-based impl in the `#19`
  seam. Honors `--share-mode ro/rw`. Reuses tested RPC.
- **Option 2 — Reuse pairing/RPC internally.** Auto-pair browser with its own
  loopback host; one code path for A+B. Less code, but wasteful
  (browser→loopback→browser via WebRTC for every op/media) and conflates "owner"
  with "guest" (auth + `rw` semantics muddled).

**Chosen: Option 1.**

---

## Hosted signalling (Mode B) — decision

Pair-by-code needs an Internet-capable signaler. Existing hot seam
(`wss://siegu.io/ws` default, `wss://` normalization, code/token pairing) is kept;
this plan wires the client + host `--server <wss-url>` plumbing while the **actual
hosted signaler is a separate (deferred) infra task**. The LAN/loopback path keeps
working today.

---

## Phased implementation

### Phase 1 — Foundation: runtime mode + shared Backend wiring
- `src/services/runtime.ts`: `detectMode(): Promise<'tauri'|'webHost'|'guest'|'onboarding'>`
  using the table above.
- `createBackend.ts`: extend picker to `'tauri' | 'webHost' | 'guest'`.
- **Stores refactor** (core step): inject a shared `Backend` (built once in
  `main.ts`) into `search/app/scan/models/albums` stores instead of direct
  `@/services/tauri` calls.
- **Media routing**: `resolveMediaUrl(id, kind)` helper calling `backend.mediaUrl()`;
  update `MediaThumbnail.vue:36`, `MediaCard.vue:130`, `MediaViewer.vue:512`,
  `MapView.vue:104`, `CollectionsView.vue:759` off `useMediaUrl()`.
- Commit the Mode A Option-1 decision so stores compile against all three backends.

### Phase 2 — Mode B: guest boot in the served `src/` bundle
- `main.ts`: when mode is guest, build `GuestClient` + `createPeerTransport('/ws', session
  from #code.token)` → `guestBackend()` (reuse `webclient/src/main.ts` +
  `peer.ts:39` / `guest.ts`).
- `App.vue`: run Tauri-only `onMounted` boot only in desktop mode; guest boots from the
  peer manifest.
- Settings tile → upsell/plan view in guest/webHost (not `SettingsView`,
  `App.vue:210-214`).
- Replace the mDNS `webOnly` hint with a proper web pairing entry (paste `#code.token`
  or a Share link).

### Phase 3 — Mode A: WebHost backend (Rust + TS)
- Rust (`web.rs`): `POST /rpc` bridging to `rpc::dispatch` under a session token; `GET
  /media/*` and `/thumb/*` auth'd routes; honor `--share-mode ro/rw`.
- `src/services/backend/webHostBackend.ts`: `fetch`-based `Backend`.
- Session auth + `/session` self-probe as webHost detection + auth handshake.

### Phase 4 — Mode B multi-tenant signalling
- `siegu-cli web --server <wss://signal>` + desktop "Share" → hosted signaler;
  browser `getConfiguredSignalingUrl()`.
- Shared / album-scoped sharing so a guest only streams what's shared
  (`mesh.rs:1567-1596` album scope exists).
- Infra (hosted signaler, runbook, auth/token + rate limits) is a **separate task**.

### Phase 5 — Hardening across modes
- Auth for webHost; token rotation for guest rooms; scope-check every
  `FetchMediaRequest`/`CommandRequest`; per-mode onboarding/landing; e2e: desktop
  Tauri, webHost self-serve, guest pair-and-stream.

---

## Phase tracking (GitHub issues)
- #24 PHASE-1 — runtime mode detection + shared Backend wiring
- #25 PHASE-2 — Mode B guest boot in served `src` bundle
- #26 PHASE-3 — Mode A WebHost backend (Rust HTTP-RPC + TS)
- #27 PHASE-4 — Mode B hosted `wss://` signalling plumbing
- #28 PHASE-5 — hardening across modes

## Related issues
- #19 Full-parity web client over WebRTC RPC (replaces `webclient/`)
- #14 Stripe + license-key entitlements + hosted signalling layer
- #16 Shareable albums — live link + ephemeral web view
- #21 Upsell guests to install the native app
- #8 Landing page / connect-device upsell
- #6 Guest demo mode
- #7 Demo dataset
