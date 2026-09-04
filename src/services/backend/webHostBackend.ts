/**
 * WebHost-mode `Backend` adapter (#24/#26, Mode A).
 *
 * The browser is the OWNER of a library mounted on a `siegu web` instance, so it
 * talks to its own Rust host over HTTP rather than pairing over WebRTC (Option 1
 * from `docs/architecture-web-modes.md`).
 *
 * Wire contract (implemented on the Rust side in PHASE-3, #26; auth in #28):
 *   - `POST /rpc`        body `{ name, payload }` → `{ ok, result?, error? }`
 *                        (mirrors `crates::rpc::dispatch`, `--share-mode ro/rw`),
 *                        requires `Authorization: Bearer <webToken>`
 *   - `GET  /thumb/{id}?token=<webToken>` → thumbnail bytes (image mime)
 *   - `GET  /media/{id}?token=<webToken>` → original bytes   (image/video mime)
 *
 * `webToken` (from `/session`) is required once #28 gates the host routes.
 *
 * The TS side is fully functional once those routes exist; before then, calls
 * reject with a clear "not implemented / route unavailable" error.
 */
import type { MediaItem, ListFilesOptions } from '@/types/media';
import type { SearchFacetsData } from '@/types/search';
import type { Backend, MediaKind } from './interface';
import { mediaCacheKey } from './interface';
import { toSnakeCaseKeys } from './rpcCasing';

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

async function rpc<T>(
  name: string,
  payload: Record<string, unknown> = {},
  webHostToken?: string,
): Promise<T> {
  let res: Response;
  try {
    const headers: Record<string, string> = { 'content-type': 'application/json' };
    if (webHostToken) headers.authorization = `Bearer ${webHostToken}`;
    res = await fetch(RPC_PATH, {
      method: 'POST',
      headers,
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

/**
 * Build an HTTP media URL for `id`. The host resolves id → file bytes, and
 * (once #28 land) requires the `?token=` query since `<img>` can't send headers.
 */
function mediaUrlFor(id: number | string, kind: MediaKind, webHostToken?: string): string {
  const path = `${kind === 'thumb' ? THUMB_PATH : MEDIA_PATH}/${String(id)}`;
  return webHostToken ? `${path}?token=${encodeURIComponent(webHostToken)}` : path;
}

export function webHostBackend(webHostToken?: string): Backend {
  const cache = new Map<string, string>();
  const rpcCall = <T>(name: string, payload: Record<string, unknown> = {}) =>
    rpc<T>(name, toSnakeCaseKeys(payload), webHostToken);

  return {
    listFiles: (options: Partial<ListFilesOptions> = {}) =>
      rpcCall<MediaItem[]>('list_files', {
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
        storedOnly: options.storedOnly ?? false,
        notStoredOnly: options.notStoredOnly ?? false,
        random: options.random ?? false,
        orderBy: options.orderBy,
        albumId: options.albumId,
      }),

    getPhotoById: (id) => rpcCall<MediaItem | null>('get_photo_by_id', { id: String(id) }),

    getPhotosByIds: (ids) =>
      rpcCall<MediaItem[]>('get_photos_by_ids', {
        ids: ids.map((id) => String(id)),
      }),

    getPhotoEncodedBatch: (ids) =>
      rpcCall<Record<number, string>>('get_photo_encoded_batch', {
        ids: ids.map((id) => String(id)),
      }),

    searchFacets: () => rpcCall<SearchFacetsData>('get_search_facets'),

    countTrash: () => rpcCall<number>('count_trash'),

    listTrash: (limit = 100) => rpcCall<MediaItem[]>('list_trash', { limit }),

    toggleFavorite: (id) => rpcCall<boolean>('toggle_favorite', { id: String(id) }),

    setFavorites: (ids, favorite) =>
      rpcCall<number>('set_favorites', {
        ids: ids.map((id) => String(id)),
        favorite,
      }),

    trashPhoto: (id) => rpcCall<boolean>('trash_photo', { id: String(id) }),

    restorePhoto: (id) => rpcCall<boolean>('restore_photo', { id: String(id) }),

    emptyTrash: () => rpcCall<number>('empty_trash'),

    request: <T = unknown>(name: string, payload: Record<string, unknown> = {}) =>
      rpcCall<T>(name, payload),

    mediaUrl: async (id, kind) => {
      const key = mediaCacheKey(id, kind);
      const cached = cache.get(key);
      if (cached) return cached;
      const url = mediaUrlFor(id, kind, webHostToken);
      cache.set(key, url);
      return url;
    },

    cachedMediaUrl: (id, kind) => cache.get(mediaCacheKey(id, kind)),

    close: () => cache.clear(),
  };
}
