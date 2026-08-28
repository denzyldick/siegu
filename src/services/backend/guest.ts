/**
 * Guest RPC backend (#19, Phase 1): talks to a hosting `siegu web` device over
 * a WebRTC data channel using the same `CommandRequest`/`CommandResponse`
 * envelope the desktop Tauri commands use. Media (thumbnails + originals) is
 * pulled on demand over the same channel and cached as blob URLs.
 *
 * Built on {@link PeerTransport} so the correlation + media logic is testable
 * with a fake transport.
 */
import { FileAssembler } from './protocol';
import type { GuestInbound, GuestOutbound } from './protocol';
import type { MediaItem, ListFilesOptions } from '@/types/media';
import type { SearchFacetsData } from '@/types/search';

/** A peer that has been handed an offer to answer. */
export interface GuestEvents {
  onOpen?: () => void;
  onClose?: () => void;
  onError?: (message: string) => void;
  /** Called when a transfer completes and a new blob URL is available. */
  onMedia?: (id: number | string, key: string, url: string) => void;
}

export class GuestClient {
  private idCounter = 0;
  private pending = new Map<
    number,
    { resolve: (v: unknown) => void; reject: (e: Error) => void }
  >();
  private caches: Array<{ key: string; url: string }> = [];
  private assemblers = new Map<number | string, FileAssembler>();
  private inflight = new Map<
    string,
    { promise: Promise<string | null>; resolve: (u: string | null) => void }
  >();
  private closed = false;

  constructor(
    private readonly transport: {
      send(msg: GuestOutbound): void;
      onMessage(handler: (msg: GuestInbound) => void): void;
      onOpen(handler: () => void): void;
      onClose(handler: () => void): void;
      isOpen(): boolean;
      close(): void;
    },
    private readonly events: GuestEvents = {},
  ) {
    transport.onOpen(() => this.events.onOpen?.());
    transport.onClose(() => {
      this.closed = true;
      this.rejectAll(new Error('Session ended'));
      this.events.onClose?.();
    });
    transport.onMessage((msg) => this.handleInbound(msg));
  }

  /** Fire-and-forget a CommandRequest; resolve on its correlated response. */
  request<T = unknown>(name: string, payload: Record<string, unknown> = {}): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      if (this.closed) return reject(new Error('Session ended'));
      const id = ++this.idCounter;
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
      this.transport.send({ type: 'CommandRequest', id, name, payload });
    });
  }

  // ── Command mirrors (subset of the Tauri surface; payloads 1:1 with rpc.rs)

  listFiles(options: Partial<ListFilesOptions> = {}): Promise<MediaItem[]> {
    return this.request<MediaItem[]>('list_files', {
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
    });
  }

  searchFacets(): Promise<SearchFacetsData> {
    return this.request<SearchFacetsData>('get_search_facets');
  }

  getPhotoById(id: number | string): Promise<MediaItem | null> {
    return this.request<MediaItem | null>('get_photo_by_id', { id: String(id) });
  }

  countTrash(): Promise<number> {
    return this.request<number>('count_trash');
  }

  listTrash(limit = 100): Promise<MediaItem[]> {
    return this.request<MediaItem[]>('list_trash', { limit });
  }

  toggleFavorite(id: number | string): Promise<boolean> {
    return this.request<boolean>('toggle_favorite', { id: String(id) });
  }

  trashPhoto(id: number | string): Promise<boolean> {
    return this.request<boolean>('trash_photo', { id: String(id) });
  }

  restorePhoto(id: number | string): Promise<boolean> {
    return this.request<boolean>('restore_photo', { id: String(id) });
  }

  emptyTrash(): Promise<number> {
    return this.request<number>('empty_trash');
  }

  // ── Media ───────────────────────────────────────────────────────────────

  /** Resolve a cached blob URL, or request it and await the transfer. */
  fetchThumb(id: number | string): Promise<string | null> {
    return this.fetchMedia(id, true);
  }

  fetchOriginal(id: number | string): Promise<string | null> {
    return this.fetchMedia(id, false);
  }

  private fetchMedia(id: number | string, thumbnail: boolean): Promise<string | null> {
    const key = `${thumbnail ? 'thumb' : 'original'}:${id}`;
    const cached = this.cachedUrl(key);
    if (cached) return Promise.resolve(cached);
    const inflight = this.inflight.get(key);
    if (inflight) return inflight.promise;
    let resolve!: (u: string | null) => void;
    const promise = new Promise<string | null>((r) => {
      resolve = r;
    });
    this.inflight.set(key, { promise, resolve });
    this.assemblers.set(
      id,
      new FileAssembler(id, (blob) => {
        const url = URL.createObjectURL(blob);
        this.caches.push({ key, url });
        this.resolveMedia(key, url);
        this.events.onMedia?.(id, key, url);
      }),
    );
    this.transport.send({ type: 'FetchMediaRequest', id, thumbnail });
    return promise;
  }

  private resolveMedia(key: string, url: string | null): void {
    const inflight = this.inflight.get(key);
    if (!inflight) return;
    this.inflight.delete(key);
    inflight.resolve(url);
  }

  cachedUrl(key: string): string | undefined {
    return this.caches.find((c) => c.key === key)?.url;
  }

  private handleInbound(msg: GuestInbound): void {
    switch (msg.type) {
      case 'CommandResponse': {
        const p = this.pending.get(msg.id);
        if (!p) return;
        this.pending.delete(msg.id);
        if (msg.ok) p.resolve(msg.result);
        else p.reject(new Error(msg.error ?? `Command ${msg.id} failed`));
        break;
      }
      case 'FileHeader': {
        this.assemblers.get(msg.id)?.header(msg.filename);
        break;
      }
      case 'FileChunk': {
        this.assemblers.get(msg.id)?.chunk(msg.index, msg.data);
        break;
      }
      case 'FileEnd': {
        this.assemblers.get(msg.id)?.end();
        this.assemblers.delete(msg.id);
        break;
      }
      case 'ViewMedia': {
        // Mirror webclient/main.ts: base64 image delivered inline.
        const id = msg.id;
        const raw = atob(msg.data);
        const bytes = new Uint8Array(raw.length);
        for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
        const url = URL.createObjectURL(new Blob([bytes], { type: msg.mime }));
        const key = `thumb:${id}`;
        this.caches.push({ key, url });
        this.resolveMedia(key, url);
        this.events.onMedia?.(id, key, url);
        break;
      }
      default:
        break;
    }
  }

  private rejectAll(err: Error): void {
    for (const p of this.pending.values()) p.reject(err);
    this.pending.clear();
  }

  close(): void {
    this.closed = true;
    this.transport.close();
    for (const c of this.caches) URL.revokeObjectURL(c.url);
    this.caches = [];
    this.assemblers.clear();
    for (const { resolve } of this.inflight.values()) resolve(null);
    this.inflight.clear();
  }
}
