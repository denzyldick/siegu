/**
 * WebHost-mode `Backend` adapter (#24/#26, Mode A).
 *
 * The browser is the OWNER of a library mounted on a `siegu web` instance, so it
 * talks to its own Rust host over HTTP rather than pairing over WebRTC (Option 1
 * from `docs/architecture-web-modes.md`).
 *
 * Wire contract (implemented on the Rust side in PHASE-3, #26):
 *   - `POST /rpc`        body `{ name, payload }` → `{ ok, result?, error? }`
 *                        (mirrors `crates::rpc::dispatch`, `--share-mode ro/rw`)
 *   - `GET  /thumb/{id}` → thumbnail bytes  (image mime)
 *   - `GET  /media/{id}` → original bytes    (image/video mime)
 *
 * The TS side is fully functional once those routes exist; before then, calls
 * reject with a clear "not implemented / route unavailable" error.
 */
import type { MediaItem, ListFilesOptions } from '@/types/media';
import type { SearchFacetsData } from '@/types/search';
import type { Backend, MediaKind } from './interface';
import { mediaCacheKey } from './interface';

const RPC_PATH = '/rpc';
const THUMB_PATH = '/thumb';
const MEDIA_PATH = '/media';

interface RpcResult<T> {
  ok: boolean;
  result?: T;
  error?: string;
}

class RpcError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'RpcError';
  }
}

async function rpc<T>(name: string, payload: Record<string, unknown> = {}): Promise<T> {
  let res: Response;
  try {
    res = await fetch(RPC_PATH, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ name, payload }),
    });
  } catch {
    throw new RpcError(
      `WebHost RPC unavailable: ${name}. The host's /rpc endpoint is not reachable.`,
    );
  }
  if (!res.ok) {
    throw new RpcError(`WebHost RPC HTTP ${res.status} for ${name}`);
  }
  const body = (await res.json()) as RpcResult<T>;
  if (!body.ok) {
    throw new RpcError(body.error ?? `RPC ${name} failed on host`);
  }
  return body.result as T;
}

/** Build an HTTP media URL for `id`. The host resolves id → file bytes. */
function mediaUrlFor(id: number | string, kind: MediaKind): string {
  return `${kind === 'thumb' ? THUMB_PATH : MEDIA_PATH}/${String(id)}`;
}

export function webHostBackend(): Backend {
  const cache = new Map<string, string>();

  return {
    listFiles: (options: Partial<ListFilesOptions> = {}) =>
      rpc<MediaItem[]>('list_files', {
        offset: options.offset ?? 0,
        limit: options.limit ?? 1000,
        query: options.query ?? '',
        favoritesOnly: options.favoritesOnly ?? false,
        videosOnly: options.videosOnly ?? false,
        personIds: options.personIds ?? [],
        location: options.location,
        tag: options.tag,
        dateFrom: options.dateFrom,
        dateTo: options.dateTo,
        camera: options.camera,
        papers: options.papers ?? false,
        nsfwOnly: options.nsfwOnly ?? false,
        random: options.random ?? false,
        orderBy: options.orderBy,
        albumId: options.albumId,
      }),

    getPhotoById: (id) => rpc<MediaItem | null>('get_photo_by_id', { id: String(id) }),

    searchFacets: () => rpc<SearchFacetsData>('get_search_facets'),

    countTrash: () => rpc<number>('count_trash'),

    listTrash: (limit = 100) => rpc<MediaItem[]>('list_trash', { limit }),

    toggleFavorite: (id) => rpc<boolean>('toggle_favorite', { id: String(id) }),

    trashPhoto: (id) => rpc<boolean>('trash_photo', { id: String(id) }),

    restorePhoto: (id) => rpc<boolean>('restore_photo', { id: String(id) }),

    emptyTrash: () => rpc<number>('empty_trash'),

    mediaUrl: async (id, kind) => {
      const key = mediaCacheKey(id, kind);
      const cached = cache.get(key);
      if (cached) return cached;
      const url = mediaUrlFor(id, kind);
      cache.set(key, url);
      return url;
    },

    cachedMediaUrl: (id, kind) => cache.get(mediaCacheKey(id, kind)),

    close: () => cache.clear(),
  };
}
