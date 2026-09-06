import { ref, watch, type Ref } from 'vue';
import { getMediaServerPort } from '@/services/tauri';
import { isVideoFile } from '@/types/media';
import { resolveBackendMedia } from '@/services/backend/mediaRegistry';
import type { MediaItem } from '@/types/media';
import type { MediaKind } from '@/services/backend/interface';

const sharedPort = ref<number | null>(null);
let portPromise: Promise<number | null> | null = null;

const mediaCache = new Map<string, string>();
const MEDIA_CACHE_MAX = 2000;

function cacheKey(item: MediaItem, kind: MediaKind): string {
  return `${kind}:${item.id}`;
}

function cacheGet(item: MediaItem, kind: MediaKind): string | null {
  return mediaCache.get(cacheKey(item, kind)) ?? null;
}

function cacheSet(item: MediaItem, kind: MediaKind, url: string): void {
  if (mediaCache.size >= MEDIA_CACHE_MAX) {
    mediaCache.delete(mediaCache.keys().next().value as string);
  }
  mediaCache.set(cacheKey(item, kind), url);
}

/**
 * Drop a single media item's cached URL(s) so a later resolution re-resolves
 * instead of returning a stale/revoked URL. Used by the viewer's bounded
 * preload window when the far neighbour is evicted (guest mode revokes the
 * blob, so the cached string must go with it).
 */
export function invalidateMediaUrl(itemId: string | number, kind?: MediaKind): void {
  const id = String(itemId);
  if (kind) {
    mediaCache.delete(`${kind}:${id}`);
    return;
  }
  mediaCache.delete(`thumb:${id}`);
  mediaCache.delete(`original:${id}`);
}

async function ensurePort(): Promise<number | null> {
  if (sharedPort.value !== null) return sharedPort.value;
  if (portPromise !== null) return portPromise;

  portPromise = (async () => {
    try {
      sharedPort.value = (await getMediaServerPort()) ?? null;
      return sharedPort.value;
    } catch (error) {
      console.error('[useMediaUrl] Failed to get media server port:', error);
      portPromise = null;
      return null;
    }
  })();

  return portPromise;
}

export function useMediaUrl() {
  void ensurePort();

  // ── Tauri-only: media streams from the local media-server HTTP routes (#10).
  //    Only meaningful on desktop; in browser modes the port stays null and
  //    components must use the mode-aware `thumbSrc`/`originalSrc`/`videoSrc`
  //    below, which delegate to the active Backend (`/media|/thumb` on webHost,
  //    WebRTC blob on guest).

  function videoUrl(location: string): string | null {
    if (!sharedPort.value || !location) return null;
    const encoded = encodeURIComponent(location);
    return `http://127.0.0.1:${sharedPort.value}/media/${encoded}`;
  }

  function imageUrl(location: string): string | null {
    if (!sharedPort.value || !location) return null;
    const encoded = encodeURIComponent(location);
    return `http://127.0.0.1:${sharedPort.value}/media/${encoded}`;
  }

  function thumbUrl(location: string): string | null {
    if (!sharedPort.value || !location) return null;
    const encoded = encodeURIComponent(location);
    return `http://127.0.0.1:${sharedPort.value}/thumb/${encoded}`;
  }

  // Evicted (view-only) items stream through the media server's /remote
  // route, which pulls bytes from the peer on demand (#10).
  function remoteImageUrl(id: string | number): string | null {
    if (!sharedPort.value) return null;
    return `http://127.0.0.1:${sharedPort.value}/remote/${encodeURIComponent(String(id))}`;
  }

  function remoteThumbUrl(id: string | number): string | null {
    if (!sharedPort.value) return null;
    return `http://127.0.0.1:${sharedPort.value}/remote/thumb:${encodeURIComponent(String(id))}`;
  }

  function isVideo(location: string): boolean {
    return isVideoFile(location);
  }

  // ── Mode-aware media resolution (all modes) ───────────────────────────────
  // The single seam for rendering media. Components should call these instead
  // of reaching into the Tauri-only URL helpers above. Resolution:
  //   - inline `encoded` bytes (base64) always win when present — they render
  //     identically in tauri, webHost and guest.
  //   - webHost / guest: delegate to the active Backend `mediaUrl(id, kind)`
  //     (webHost → `/thumb|/media/{id}?token=`, guest → WebRTC blob URL).
  //   - tauri: local media-server URL by `location`.
  async function mediaSrc(item: MediaItem, kind: MediaKind): Promise<string | null> {
    if (!item) return null;
    if (item.encoded) return item.encoded;
    const cached = cacheGet(item, kind);
    if (cached) return cached;
    const backendUrl = await resolveBackendMedia(item, kind);
    if (backendUrl) {
      cacheSet(item, kind, backendUrl);
      return backendUrl;
    }
    await ensurePort();
    const location = item.location;
    if (!location) return null;
    const url = (kind === 'thumb' ? thumbUrl(location) : imageUrl(location)) ?? null;
    if (url) cacheSet(item, kind, url);
    return url;
  }

  function thumbSrc(item: MediaItem | null | undefined): Promise<string | null> {
    return mediaSrc(item as MediaItem, 'thumb');
  }

  function originalSrc(item: MediaItem | null | undefined): Promise<string | null> {
    return mediaSrc(item as MediaItem, 'original');
  }

  function videoSrc(item: MediaItem | null | undefined): Promise<string | null> {
    return mediaSrc(item as MediaItem, 'original');
  }

  // Reactive binding for templates: resolves `kind` media for `item` whenever
  // the item changes, returning a `Ref<string | undefined>` safe to drop straight
  // into an `<img>/<video>` `:src`. Works across all modes (webHost/guest/tauri).
  function mediaSrcRef(
    item: Ref<MediaItem | null | undefined>,
    kind: MediaKind,
  ): Ref<string | undefined> {
    const src = ref<string | undefined>(undefined);
    watch(
      item,
      async (val) => {
        src.value = (await mediaSrc(val as MediaItem, kind)) ?? undefined;
      },
      { immediate: true },
    );
    return src;
  }

  return {
    port: sharedPort,
    ensurePort,
    videoUrl,
    imageUrl,
    thumbUrl,
    remoteImageUrl,
    remoteThumbUrl,
    isVideo,
    mediaSrc,
    thumbSrc,
    originalSrc,
    videoSrc,
    mediaSrcRef,
  };
}
