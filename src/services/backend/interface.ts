/**
 * A single seam between the local Tauri app and a browser guest (#19, Phase 1).
 *
 * Both implementations expose the same operations, so the UI layer can be
 * written once and switched by build/runtime mode. Data ops mirror the
 * `get_*`/`list_*` backend commands; media is resolved per-item id to a URL
 * string suitable for `<img>`/`<video>` src.
 *
 * - `tauriBackend()`  — fetches media via the local media-server HTTP routes.
 * - `guestBackend()`  — fetches media over the WebRTC RPC blob cache.
 */
import type { ListFilesOptions, MediaItem } from '@/types/media';
import type { SearchFacetsData } from '@/types/search';

export type MediaKind = 'thumb' | 'original';

export interface Backend {
  // ── data ────────────────────────────────────────────────────────────────
  listFiles(options: Partial<ListFilesOptions>): Promise<MediaItem[]>;
  getPhotoById(id: number | string): Promise<MediaItem | null>;
  searchFacets(): Promise<SearchFacetsData>;
  countTrash(): Promise<number>;
  listTrash(limit?: number): Promise<MediaItem[]>;
  toggleFavorite(id: number | string): Promise<boolean>;
  trashPhoto(id: number | string): Promise<boolean>;
  restorePhoto(id: number | string): Promise<boolean>;
  emptyTrash(): Promise<number>;

  // Batch reads/mutations used by the gallery: mirrors the host RPCs
  // `get_photos_by_ids`, `get_photo_encoded_batch`, `set_favorites` so the
  // browser data plane (`@/services/invoke`) can serve them in webHost/guest.
  getPhotosByIds(ids: Array<string | number>): Promise<MediaItem[]>;
  getPhotoEncodedBatch(ids: number[]): Promise<Record<number, string>>;
  setFavorites(ids: Array<string | number>, favorite: boolean): Promise<number>;

  /**
   * Generic command dispatch (mirrors the host RPC surface). `name` is the
   * Tauri/RPC command name; `payload` its args. Implementations return the raw
   * resolved value as-is. UI code typically reaches here through
   * `@/services/invoke`, which re-wraps JSON-string command results.
   */
  request<T = unknown>(name: string, payload?: Record<string, unknown>): Promise<T>;

  // ── media (by id → src URL) ────────────────────────────────────────────
  /** Resolve (and cache) a media src URL for a photo. */
  mediaUrl(id: number | string, kind: MediaKind): Promise<string | null>;
  /** Sync lookup of an already-cached URL, or null if not loaded yet. */
  cachedMediaUrl(id: number | string, kind: MediaKind): string | undefined;

  close(): void;
}

export type BackendMode = 'tauri' | 'webHost' | 'guest';

/**
 * The three runtime UI modes (#24):
 *  - `tauri`    — native desktop Tauri app (unchanged behavior)
 *  - `webHost`  — browser is the OWNER of a library mounted on a `siegu web`
 *                 instance (Mode A); full GUI over HTTP/RPC, no pairing
 *  - `guest`    — browser PAIRS by code + token with a remote/desktop Siegu
 *                 and streams media over WebRTC (Mode B, web.whatsapp.com model)
 */
export type RuntimeMode = 'onboarding' | BackendMode;

/** Media cache key used by `cachedMediaUrl`/`mediaUrl`. */
export function mediaCacheKey(id: number | string, kind: MediaKind): string {
  return `${kind}:${String(id)}`;
}
