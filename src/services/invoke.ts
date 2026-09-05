/**
 * Browser-aware `invoke` (#24, Phase 1 data plane).
 *
 * In a Tauri shell this is a thin pass-through to `@tauri-apps/api/core.invoke`.
 * In a plain browser (webHost/guest modes) it serves the commands the `siegu
 * web` host supports through the registered `Backend`, and returns graceful,
 * shape-correct defaults for the rest, so the full Vue UI can run against the
 * host without Tauri. Components and `@/services/tauri` import `invoke` from
 * here instead of the Tauri package; the resolved values mirror each Rust
 * command's return contract (JSON-string commands stay JSON strings, raw
 * booleans/numbers stay raw).
 */
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Backend } from '@/services/backend/interface';
import { activeMediaBackend } from '@/services/backend/mediaRegistry';
import type { ListFilesOptions } from '@/types/media';
import { STRINGIFY_RESULT } from 'shared/generated/rpc-commands';

/** Mirrors the `isTauri` check from `@/services/tauri` without importing it
 *  (that module imports `invoke` from here, so importing it would cycle). */
export const isTauriRuntime =
  typeof window !== 'undefined' &&
  typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !==
    'undefined';

/** Vitest mocks `@tauri-apps/api/core` and the event module; in that env the
 *  data plane must consult the mocked invoke instead of the browser shim. */
const inTestMode = typeof import.meta !== 'undefined' && import.meta.env?.MODE === 'test';

/** True when the real Tauri IPC is available (desktop app under test/browser
 *  mocks included); false only in a genuine plain-browser webHost/guest run. */
export const tauriIpcActive = isTauriRuntime || inTestMode;

type InvokeArgs = Record<string, unknown>;

const BACKEND_REGISTRATION_TIMEOUT_MS = 8000;

/** Wait for `initRuntime` to register the active backend. Child components mount
 *  (and fire data calls) before App.vue's async onMounted finishes mode
 *  detection, so the browser data plane must tolerate a small registration
 *  window instead of failing on the first `list_files`. */
function waitForBackend(timeoutMs: number = BACKEND_REGISTRATION_TIMEOUT_MS): Promise<Backend> {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    const poll = (): void => {
      const backend = activeMediaBackend();
      if (backend) {
        resolve(backend);
        return;
      }
      if (Date.now() - start > timeoutMs) {
        reject(new Error('invoke: no backend registered (runtime not initialized)'));
        return;
      }
      setTimeout(poll, 25);
    };
    poll();
  });
}

function asStringArray(value: unknown): Array<string | number> {
  if (!Array.isArray(value)) return [];
  return value.map((v) => (typeof v === 'number' || typeof v === 'string' ? v : String(v)));
}

function asNumberArray(value: unknown): number[] {
  if (!Array.isArray(value)) return [];
  return value.map((v) => Number(v));
}

function num(value: unknown): number | undefined {
  const n = Number(value);
  return Number.isFinite(n) ? n : undefined;
}

function normalizeListArgs(args?: InvokeArgs): Partial<ListFilesOptions> {
  const a = args ?? {};
  return {
    offset: num(a.offset) ?? 0,
    limit: num(a.limit) ?? 1000,
    query: typeof a.query === 'string' ? a.query : '',
    scan: a.scan === true,
    favoritesOnly: a.favoritesOnly === true,
    videosOnly: a.videosOnly === true,
    personIds: Array.isArray(a.personIds)
      ? (a.personIds as string[])
      : Array.isArray(a.personIds_item)
        ? (a.personIds_item as string[])
        : undefined,
    personMatch: a.personMatch === 'and' || a.personMatch === 'or' ? a.personMatch : undefined,
    personAlone: a.personAlone === true,
    location: typeof a.location === 'string' ? a.location : undefined,
    tag: typeof a.tag === 'string' ? a.tag : undefined,
    dateFrom: typeof a.dateFrom === 'string' ? a.dateFrom : undefined,
    dateTo: typeof a.dateTo === 'string' ? a.dateTo : undefined,
    hasFaces: a.hasFaces === true,
    aestheticsMin: typeof a.aestheticsMin === 'number' ? a.aestheticsMin : null,
    camera: typeof a.camera === 'string' ? a.camera : undefined,
    papers: a.papers === true,
    nsfwOnly: a.nsfwOnly === true,
    storedOnly: a.storedOnly === true,
    notStoredOnly: a.notStoredOnly === true,
    random: a.random === true,
    orderBy: typeof a.orderBy === 'string' ? a.orderBy : undefined,
    albumId: typeof a.albumId === 'string' ? a.albumId : undefined,
  };
}

/**
 * Generic backend command dispatch for the browser data plane. Resolves the
 * registered `Backend.request`, then JSON-stringifies the result for the
 * commands in {@link STRINGIFY_RESULT} to match the Tauri contract.
 */
function requestCommand(name: string, args?: InvokeArgs): Promise<unknown> {
  return waitForBackend()
    .then((backend) => backend.request(name, args ?? {}))
    .then((result) => (STRINGIFY_RESULT.has(name) ? JSON.stringify(result) : result));
}

/** Commands served through the active `Backend` in a plain browser. */
const browserHandlers: Record<string, (args?: InvokeArgs) => Promise<unknown>> = {
  // Convenience pass-throughs that normalize args to the typed Backend methods
  // (identical wire shape to the generic `request` path below, but reused by
  // the rest of the data plane).
  list_files: async (args) =>
    JSON.stringify(await (await waitForBackend()).listFiles(normalizeListArgs(args))),
  get_photo_by_id: async (args) =>
    JSON.stringify(await (await waitForBackend()).getPhotoById(String(args?.id))),
  get_photos_by_ids: async (args) =>
    JSON.stringify(await (await waitForBackend()).getPhotosByIds(asStringArray(args?.ids))),
  get_photo_encoded_batch: async (args) => {
    const batch = await (await waitForBackend()).getPhotoEncodedBatch(asNumberArray(args?.ids));
    return JSON.stringify(batch);
  },
  get_search_facets: async () => JSON.stringify(await (await waitForBackend()).searchFacets()),
  search_facets: async () => JSON.stringify(await (await waitForBackend()).searchFacets()),
  count_trash: async () => (await waitForBackend()).countTrash(),
  list_trash: async (args) =>
    JSON.stringify(await (await waitForBackend()).listTrash(num(args?.limit) ?? 100)),
  toggle_favorite: async (args) => (await waitForBackend()).toggleFavorite(String(args?.id)),
  set_favorites: async (args) =>
    (await waitForBackend()).setFavorites(asStringArray(args?.ids), args?.favorite === true),
  trash_photo: async (args) => (await waitForBackend()).trashPhoto(String(args?.id)),
  restore_photo: async (args) => (await waitForBackend()).restorePhoto(String(args?.id)),
  empty_trash: async () => (await waitForBackend()).emptyTrash(),
  get_person_photos: async (args) =>
    JSON.stringify(
      await (
        await waitForBackend()
      ).listFiles({
        personIds: [String(args?.personId ?? args?.fromPerson ?? '')],
        offset: num(args?.offset) ?? 0,
        limit: num(args?.limit) ?? 200,
      }),
    ),
  // Everything else the host supports (albums, people, config, directories,
  // models/status, storage, trash-permanent, housekeeping) flows through the
  // generic Backend.request seam.
  list_albums: (args) => requestCommand('list_albums', args),
  get_album: (args) => requestCommand('get_album', args),
  get_album_sections: (args) => requestCommand('get_album_sections', args),
  get_album_contents: (args) => requestCommand('get_album_contents', args),
  get_clip_categories: (args) => requestCommand('get_clip_categories', args),
  create_album: (args) => requestCommand('create_album', args),
  create_smart_album: (args) => requestCommand('create_smart_album', args),
  update_smart_album_rule: (args) => requestCommand('update_smart_album_rule', args),
  rename_album: (args) => requestCommand('rename_album', args),
  delete_album: (args) => requestCommand('delete_album', args),
  clear_dismissed_trips: (args) => requestCommand('clear_dismissed_trips', args),
  sync_trips: (args) => requestCommand('sync_trips', args),
  add_album_items: (args) => requestCommand('add_album_items', args),
  remove_album_items: (args) => requestCommand('remove_album_items', args),
  reorder_album: (args) => requestCommand('reorder_album', args),
  get_people: (args) => requestCommand('get_people', args),
  get_unnamed_faces: (args) => requestCommand('get_unnamed_faces', args),
  get_person_faces: (args) => requestCommand('get_person_faces', args),
  get_faces_for_photo: (args) => requestCommand('get_faces_for_photo', args),
  get_top_tags: (args) => requestCommand('get_top_tags', args),
  assign_name_to_face: (args) => requestCommand('assign_name_to_face', args),
  delete_face: (args) => requestCommand('delete_face', args),
  merge_people: (args) => requestCommand('merge_people', args),
  rename_person: (args) => requestCommand('rename_person', args),
  get_photo_ocr: (args) => requestCommand('get_photo_ocr', args),
  get_photo_transcript: (args) => requestCommand('get_photo_transcript', args),
  get_model_timings: (args) => requestCommand('get_model_timings', args),
  get_model_timing_averages: (args) => requestCommand('get_model_timing_averages', args),
  find_duplicates: (args) => requestCommand('find_duplicates', args),
  duplicate_stats: (args) => requestCommand('duplicate_stats', args),
  trash_duplicate_members: (args) => requestCommand('trash_duplicate_members', args),
  get_heatmap_data: (args) => requestCommand('get_heatmap_data', args),
  day_counts: (args) => requestCommand('day_counts', args),
  list_objects: (args) => requestCommand('list_objects', args),
  get_location_names: (args) => requestCommand('get_location_names', args),
  list_directories: (args) => requestCommand('list_directories', args),
  is_initialized: (args) => requestCommand('is_initialized', args),
  get_config: (args) => requestCommand('get_config', args),
  save_config: (args) => requestCommand('save_config', args),
  add_directory: (args) => requestCommand('add_directory', args),
  remove_directory: (args) => requestCommand('remove_directory', args),
  remove_directory_full: (args) => requestCommand('remove_directory_full', args),
  mark_onboarding_complete: (args) => requestCommand('mark_onboarding_complete', args),
  get_last_scan_time: (args) => requestCommand('get_last_scan_time', args),
  get_unindexed_count: (args) => requestCommand('get_unindexed_count', args),
  get_max_photo_rowid: (args) => requestCommand('get_max_photo_rowid', args),
  get_indexing_status: (args) => requestCommand('get_indexing_status', args),
  get_storage_usage: (args) => requestCommand('storage_usage', args),
  check_models: (args) => requestCommand('check_models', args),
  get_model_capabilities: (args) => requestCommand('get_model_capabilities', args),
  delete_photo_permanently: (args) => requestCommand('delete_photo_permanently', args),
  cleanup_database: (args) => requestCommand('cleanup_database', args),
};

/** Graceful, shape-correct defaults for genuinely host-less / desktop-only
 *  commands (browser mode). Commands the host does implement live in
 *  `browserHandlers` above instead of here. */
const browserFallbacks: Record<string, unknown> = {
  get_os: 'Browser',
  get_system_dark_mode:
    typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches,
  ping_signaling: 0,
  get_media_server_port: 0,
  get_logs: '[]',
  get_models_loaded: false,
  resolve_photo_locations: undefined,

  // Desktop-only (sync/mesh/wallpaper/file-read) — not part of the host RPC.
  set_wallpaper: undefined,
  fetch_original: undefined,
  initialize_sync_folder: undefined,
  request_start_sync: undefined,
  enter_view_only: undefined,
  list_devices: '[]',
  remove_device: undefined,
  rename_device: undefined,
  generate_pairing_codes: '{}',
  hash_pairing_code: '',

  // Indexing / model lifecycle: the host has no live job queue. These resolve
  // as no-ops so the on-device pipeline UI doesn't throw in browser mode.
  abort_indexing: undefined,
  pause_indexing: undefined,
  resume_indexing: undefined,
  index_faces: undefined,
  analyze_photo: undefined,
  analyze_photo_model: undefined,
  analyze_model: undefined,
  download_models: undefined,
  reload_models: undefined,
  unload_models: undefined,
  clear_logs: undefined,

  // Pro license verification is desktop-only (needs the Worker + secrets).
  // Resolve a shape-correct "not found" so the Settings Pro section doesn't
  // throw in browser / guest mode.
  verify_pro_email: '{"ok":false,"paid":false,"verified":false}',
  send_pro_verification: '{"ok":false,"paid":false,"verified":false}',
};

export async function invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
  if (tauriIpcActive) {
    return args ? tauriInvoke<T>(command, args) : tauriInvoke<T>(command);
  }
  const handler = browserHandlers[command];
  if (handler) return handler(args) as Promise<T>;
  if (command in browserFallbacks) return browserFallbacks[command] as T;
  throw new Error(`invoke: command '${command}' is unavailable in browser mode`);
}

/** Tauri event subscriptions are a no-op in a plain browser (`listen` throws
 *  there because it relies on Tauri's `transformCallback`). */
export async function listen<T>(
  event: string,
  handler: (event: { payload: T }) => void,
): Promise<UnlistenFn> {
  if (!tauriIpcActive) return () => {};
  return tauriListen<T>(event, handler);
}
