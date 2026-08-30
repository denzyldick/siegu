# Demo datasets — validation report & bug findings

This document records what the demo-dataset validation **proves**, and — more
importantly for the upcoming cross-platform pass — every **bug / risk** surfaced
while writing the black-box CLI tests (`crates/siegu-cli/tests/demo_seed.rs`).

The goal of collecting these here: we need to check whether each issue also
reproduces in the **web**, **desktop (Tauri)**, and **iOS** builds of `siegu`,
and plan fixes accordingly. Each entry has an `#impact-platforms` line mapping to
`web` / `desktop` / `ios` / `cli` so the review pass can be mechanical.

---

## 1. What is validated

The test suite runs the **real compiled `siegu` binary** as an external API
(`CARGO_BIN_EXE_siegu`), against a throwaway config dir, then reads the
produced `siegu.db` with `rusqlite` and asserts the extracted state matches the
bundled `demos/` source assets.

### Group A — deterministic, offline (always run)
| Test | Proves |
|---|---|
| `seed_demo_produces_source_matching_library` | one album per category; `photo` row count == source image count; album membership (`album_item`) == source count; no dangling `album_item -> photo` refs |
| `seed_demo_generates_thumbnails_for_every_photo` | every photo gets a `encoded` thumbnail; thumbs are base64 JPEG (`data:image/jpeg...`) |
| `seed_demo_is_idempotent` | re-seed adds 0 photos, reuses albums, never duplicates them |
| `unrecognized_category_is_reported_not_fatal` | unknown slug warns and seeds nothing, exit 0 |
| `seeded_albums_are_queryable_by_the_album_join_features_use` | the `album_item JOIN photo` path features use to power the album view returns results |
| `seed_demo_combined_album_contains_every_photo` | the combined "My Photos" album contains all 46 items (self-heals older seeds) |
| `seed_demo_videos_gain_poster_thumbnails_and_never_seed_posts` | video clips get a poster `encoded` thumb and the `*_poster.jpg` files are never seeded as photos |

### Group B — ML extraction (`#[ignore]`, needs ONNX models; self-skips otherwise)
| Test | Proves |
|---|---|
| `analyze_persists_objects_and_marks_indexed` | `analyze all --headless` produces `object` (tags) rows and marks photos `indexed=2` |
| `analyze_persists_people_from_faces` | `faces` + `people` rows recorded for the people demo |
| `analyze_persists_aesthetics_and_face_count_properties` | `aesthetics_score` and the `face_count` property persisted |
| `extracted_tags_are_searchable_by_the_query_features_use` | extracted tags attach to real photos so search/facets can find them |

### Group C — how it runs
- ML group skips when ONNX models are absent, mirroring
  `siegu_core::ml_engine::models::test_models_dir` and `src-tauri/src/ml.rs`.
  CI runs them with `--ignored` once models are downloaded (`docs/ci.md`).
- Reading the DB read-only via `rusqlite` is safe; nothing is mutated.

**Result (current):** `cargo test -p siegu-cli` green — **7 deterministic
integration tests** in `demo_seed.rs` passing (Group A + the combined-album and
video-poster tests + `pretty_category` unit); **4 Group B ML tests are
ignored** until models are present. With ONNX models present, **all 4 Group B
ML tests pass** too against the 46-item demo.

### Group B validation results (run once against the real models)
All four ML tests were executed for real (ONNX models present) and pass:
- `analyze_persists_objects_and_marks_indexed` → object/tag rows written,
  photos marked `indexed=2`.
- `analyze_persists_people_from_faces` → `faces` and `people` rows for the people demo.
- `analyze_persists_aesthetics_and_face_count_properties` → `aesthetics_score`
  and `face_count` property persisted.
- `extracted_tags_are_searchable_by_the_query_features_use` → tags attach to
  real photos and are reachable via search/facets.

**Model-dependent tests beyond the demo (also run, all green):**
- `ml_engine::models::tests::session_pool_allows_concurrent_locks` (YuNet) —
  proves two concurrent locks get distinct pooled sessions (real model).
- `ml_engine::whisper::tests::test_decoder_model_metadata` +
  `test_whisper_transcribe_real_video` — encoder/decoder session I/O metadata and
  a full real-video transcription. Note these read models from
  `$HOME/.config/io.denzyl.siegu/models` (not `test_models/`) and the video path
  was corrected to `~/Pictures/Takeout/Google Photos/…`.

---

## 2. Bugs / risks found while writing the tests

### BUG-1 — Seeder is silent for already-indexed categories (re-seed UX)
**Finding:** On a re-run, categories whose image set is unchanged print *no*
`SEEDED <cat> ...` line at all — they fall into the `then continue` branch
(`main.rs:1548-1551`, `cli_warn!("no images seeded for demo category")`) and the
only feedback is a single generic `DEMO SEED DONE ... photos_added=0`
(`main.rs:1575`). The original test asserted the re-seed re-printed albums, which
failed — that's how the gap surfaced.
**Impact:** Operator can't tell *which* category was cached vs. actually
(re)seeded. Low severity, but confusing in CI/logs.
**#impact-platforms:** `cli` (seed runs only on a CLI). No web/desktop/ios path.
**Fix to plan:** emit a per-category status line (`CACHED <cat> photos=0`) or a
`SEEDED ... photos=0` line instead of only the aggregate.

### BUG-2 — Default demo-root is a build-time path (packaged installs fail)
**Finding:** `resolve_demo_root` (`main.rs:1429-1444`) falls back to
`env!("CARGO_MANIFEST_DIR")/../../demos` — that path is baked in at **compile
time**. For a dev build it resolves fine, but for a packaged artifact
(`cargo install`, a release bundle, a web/desktop/ios-served bundle) the
`demos/` tree will not exist at that path at runtime.
**Impact:** On packaged installs `seed-demo` with no `--demos-root` /
`SIEGU_DEMO_ROOT` finds nothing and silently seeds 0 photos across every
category (each just `continue`s). Data would be *missing*, not wrong.
**#impact-platforms:** `cli` primary. **Also `web` / `desktop` / `ios`** — any
platform that ships a binary without the repo-relative source tree. If demo
assets are later served from a bundle, this must be reworked to a
runtime/relative resource path (e.g. `resolved_exe_dir()/../demos` or embedded).
**Fix to plan:** resolve relative to the executable's directory (or embed assets
via `include_bytes!` / a platform resource bundle) instead of
`CARGO_MANIFEST_DIR`.

### BUG-3 — No allowlist of demo category slugs
**Finding:** `--demos` accepts any directory name under `demos/` (`main.rs:1460`)
with no validation against known slugs; unknown ones just warn and are skipped.
`pretty_category` passes unknown slugs through unchanged as the album name.
**Impact:** Misspelling (`landscape` vs `landscapes`) seeds nothing without a
helpful error listing valid choices. Low severity.
**#impact-platforms:** `cli` only (flag parsing is CLI-only today). If a demos
picker is later exposed in web/desktop/ios UI, the slug list must be shared.
**Fix to plan:** validate against `["landscapes","people","cities","food"]` and
print valid choices on mismatch.

### BUG-4 — Album dedup is by name, so slug→pretty collisions merge albums
**Finding:** idempotency reuses albums by `album.name` (`main.rs:1483-1487`).
The bundled 4 slugs map to 4 distinct pretty names, so it's correct today. But
nothing prevents two *different* slugs from mapping to one name (e.g. a future
`city` slug would collide with `cities` → "Cities & Travel") and silently share
an album.
**Impact:** latent; no live repro with current data. Would cause wrong album
grouping if slugs are added.
**#impact-platforms:** `cli` seeder logic; the resulting DB would be mis-grouped
for **web / desktop / ios** album views.
**Fix to plan:** either key dedup on category slug, or make `pretty_category`
unique-per-slug and unit-test the mapping (partially covered by
`pretty_category_maps_recognised_slugs`).

### BUG-5 — Thumbnails generated synchronously during seed (latency on big sets)
**Finding:** `seed-demo` calls `generate_thumbnail` inline per photo
(`main.rs:1538-1542`). Fine at 24 assets (~ms each), but the code path is
O(N·decode+encode) and blocks the whole command.
**Impact:** not a correctness bug; scales poorly. If datasets grow or this runs
on-import in a UI thread on desktop/ios it could block.
**#impact-platforms:** `cli` today; **risk for `desktop` / `ios`** if seeding is
ever invoked from the app's main thread instead of a worker.
**Fix to plan:** keep as-is for now (bundle is small); note threading requirement
if reused from app code.

### BUG-6 — `add_directory` re-registered on every seed (no-op but noisy)
**Finding:** `cmd_seed_demo` calls `db.add_directory(&demo_root)` on every run
(`main.rs:1475`) so the demo root is (re)watched. It is idempotent in practice,
but it means a seeder introduces a persistent watched-directory side effect.
**Impact:** low; the demo library is now continuously scanned by the DB layer.
For web/desktop/ios this means every demo library ships with a watcher on its
media root.
**#impact-platforms:** `cli` seed behavior; the watcher side effect then applies
wherever that DB is opened (**web / desktop / ios**).
**Fix to plan:** decide whether demo libraries should be static (no watcher) or
watched; document the choice.

---

## 3. Review checklist for the cross-platform pass

For each **BUG-2, BUG-4, BUG-5, BUG-6** (the ones that can leak into other
platforms), confirm against the matching surface before closing:

- [ ] **web** — does the demo gallery read the same `album_item JOIN photo` data?
      (webHostBackend `listFiles({albumId})`). Does it rely on any of BUG-2’s
      path assumptions? Web serves `demos/` via a static route — verify asset
      paths survive bundling.
- [ ] **desktop (Tauri)** — Tauri bundles resources under a per-platform resource
      dir; does seeding resolve demo assets correctly there (BUG-2)? Is seeding
      called on a worker thread (BUG-5 / BUG-6)?
- [ ] **ios** — sandboxed file system; absolute build/install paths (BUG-2) will
      break; confirm demo assets are shipped in the bundle and resolved relative
      to a writable sandbox dir. Watch for BUG-6’s watcher on sandboxed roots.

No bug currently affects data *in-flight* correctness of a single seed; BUG-2
and BUG-4 are the two most likely to bite outside the CLI.

---

## 4. Proven-dead issues (do not re-investigate)
- **Random photo ids vs idempotency** — handled by guarding on `location` via
  `load_existing_paths`, exactly as `cmd_scan` does; verified by
  `seed_demo_is_idempotent`.
- **Album duplication** — verified reused, not duplicated (`seed_demo_is_idempotent`).
- **Dangling album refs** — verified 0 (`seed_demo_produces_source_matching_library`).

---

## 5. WebHost data plane — verified in a real browser

The full Vue UI (not the legacy `webclient/dist` share view) now runs in a plain
browser against a `siegu web` host over the `Browser data-plane` seam.

**Demo composition (as of this report):** **46 items** = 40 photos (picsum
800×600; 4 categories × 10: landscapes, people, cities, food) + **6 synthetic
video clips** (`demos/videos/1.mp4..6.mp4`, 640×360 H.264/AAC, no third-party
license), each with a poster `<n>_poster.jpg` stored as the video photo's
`encoded` thumbnail. A combined **"My Photos"** album is backfilled from the
whole library each seed (self-healing). Collections shows 5 categories +
"My Photos". Seeding stays in the CLI and is validated by the external-API tests.

Verified end-to-end on the seeded demo (Playwright/Chromium, 0 console errors):

- **Boot** → `GET /session` → `{ code, webToken }` → runtime mode `webHost`,
  active `Backend` registered (`RuntimeStore.initRuntime`).
- **Gallery** → `POST /rpc` `list_files` (MediaLibrary’s full arg set incl.
  `albumId`, `personIds`, filters) → 46 items; thumbnails render via
  `/thumb/{id}?token=`, video posters via the encoded thumbnail.
- **Viewer** → originals render via `/media/{id}?token=` (800px), viewer open on
  tile click; video playback works.
- **Collections** → album covers resolve via the album's `cover_photo_id`
  (fixed: previously `/thumb/{albumId}` 404'd because album tiles keyed on the
  album UUID instead of its cover photo id — `tileSrcRef` in
  `CollectionsView.vue` now maps album items to `cover_photo_id`); 4 covers
  render, 0 broken.
- No Tauri IPC is touched in the browser: `invoke` and `listen`
  (`src/services/invoke.ts`) dispatch supported host RPCs through the active
  `Backend`, and no-op / return shape-correct defaults for the rest.

### Host RPC inventory (`crates/siegu-core/src/rpc.rs` `dispatch`)
The host `/rpc` surface now mirrors the full Tauri command set so the browser
client can **manage** a `siegu web` instance, not just read it. Read-only
commands run in any share mode; mutations require the host to be started with
`--share-mode rw` (gate error:
`command '{name}' mutates the host library; restart with --share-mode rw`).

**Read (`READ_ONLY_COMMANDS`):** library (`list_files`, `get_photo_by_id`,
`get_photos_by_ids`, `get_photo_encoded_batch`), trash state (`count_trash`,
`list_trash`), search/facets (`get_search_facets` + `search_facets` alias,
`get_top_tags`, `list_objects`, `get_location_names`, `day_counts`), extractions
(`get_photo_ocr`, `get_heatmap_data`), albums (`list_albums`, `get_album`,
`get_album_sections`, `get_album_contents`, `get_clip_categories`), people
(`get_people`, `get_unnamed_faces`, `get_person_photos`, `get_person_faces`,
`get_faces_for_photo`), directories/config/status
(`list_directories`, `is_initialized`, `get_config`, `get_last_scan_time`,
`get_unindexed_count`, `get_max_photo_rowid`, `get_indexing_status`,
`storage_usage`, `check_models`, `get_model_capabilities`).

**Write (`READ_WRITE_COMMANDS`):** favorites/trash (`toggle_favorite`,
`set_favorites`, `trash_photo`, `restore_photo`, `empty_trash`,
`delete_photo_permanently`), albums (`create_album`, `create_smart_album`,
`update_smart_album_rule`, `rename_album`, `delete_album`,
`clear_dismissed_trips`, `sync_trips`, `add_album_items`, `remove_album_items`,
`reorder_album`), people (`assign_name_to_face`, `delete_face`, `merge_people`,
`rename_person`), config/directories (`save_config`, `add_directory`,
`remove_directory`, `remove_directory_full`, `mark_onboarding_complete`,
`cleanup_database`).

The frontend seam (`src/services/backend/*` + `src/services/invoke.ts`) exposes
a generic `Backend.request(name, payload)` on all three adapters (tauri / webHost
/ guest) and routes every supported command through it, JSON-stringifying the
results the Tauri commands return as strings.

### Deliberately desktop-only on the host (not in `/rpc`)
Sync/mesh (`initialize_sync_folder`, `request_start_sync`, `enter_view_only`,
`list_devices`, pairing-code commands, `fetch_original`), wallpaper
(`set_wallpaper`), and live file-read/IO commands stay Tauri-only; the browser
data plane returns shape-correct no-ops/fallbacks for these.

### Verified gaps (feed the cross-platform pass)
- **`list_files` returns `encoded: ""`** — thumbnails are never inline over
  webHost; the browser must fetch `/thumb`. Frontend handles it (mode-aware
  `mediaSrc`), but confirm desktop/ios don’t regress to relying on `encoded`
  presence (BUG-inverse: encoded size was the original lazy-load signal).
- **Command-name drift** — the Tauri command is `search_facets` but the Host RPC
  is `get_search_facets`. The seam merges both, but the two surfaces drifting is
  a maintenance hazard; drive both from one table.
- **`get_indexing_status` on the host is a proxy** — it returns the
  `get_unindexed_count` since the host has no live ML job queue.
- **`cleanup_database` requires `confirm=true`** on the host and maps to
  `wipe_all_data()` (the Tauri desktop path holds a scan guard the host lacks).
- **Child-before-parent mount race** — `MediaLibrary` fires `list_files` before
  `App.onMounted` finishes mode detection; the data plane now waits up to 8s for
  backend registration instead of failing (desktop/ios unaffected — Tauri IPC is
  synchronous-ish).
- **`listen` is a hard crash in a browser** (`transformCallback` undefined) —
  all boot-time event subscriptions (app/scan/models/sync stores) route through
  the browser-safe wrapper now; desktop path unchanged.
- **Favorites/trash and all write commands in the web demo need the host
  restarted with `--share-mode rw`.**
