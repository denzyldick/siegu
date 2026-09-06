import { describe, it, expect, beforeEach, vi } from 'vitest';
import { GuestClient } from '@/services/backend';
import type { GuestInbound, GuestOutbound } from '@/services/backend';

/** Deterministic fake PeerTransport: captures outbound, lets tests push inbound. */
class FakeTransport {
  outbound: GuestOutbound[] = [];
  private msgHandler: ((msg: GuestInbound) => void) | null = null;
  private closeHandler: (() => void) | null = null;
  isOpenValue = true;

  send(msg: GuestOutbound): void {
    this.outbound.push(msg);
  }
  onMessage(h: (msg: GuestInbound) => void): void {
    this.msgHandler = h;
  }
  onOpen(): void {
    /* no-op */
  }
  onClose(h: () => void): void {
    this.closeHandler = h;
  }
  isOpen(): boolean {
    return this.isOpenValue;
  }
  close(): void {
    this.closeHandler?.();
  }

  push(msg: GuestInbound): void {
    this.msgHandler?.(msg);
  }
}

function commandResponse(id: number, result: unknown): GuestInbound {
  return { type: 'CommandResponse', id, ok: true, result };
}

describe('GuestClient', () => {
  let transport: FakeTransport;
  let client: GuestClient;

  beforeEach(() => {
    transport = new FakeTransport();
    client = new GuestClient(transport);
    vi.restoreAllMocks();
  });

  it('correlates a CommandResponse to its request', async () => {
    const p = client.request('count_trash');
    const sent = transport.outbound[0] as Extract<GuestOutbound, { type: 'CommandRequest' }>;
    expect(sent.type).toBe('CommandRequest');
    expect(sent.name).toBe('count_trash');
    transport.push(commandResponse(sent.id, 7));
    await expect(p).resolves.toBe(7);
  });

  it('resolves concurrent requests to the correct responses', async () => {
    const a = client.request<number>('count_trash');
    const b = client.request<number>('count_trash', { limit: 3 });
    const idA = (transport.outbound[0] as { id: number }).id;
    const idB = (transport.outbound[1] as { id: number }).id;
    expect(idA).not.toBe(idB);
    transport.push(commandResponse(idB, 99));
    transport.push(commandResponse(idA, 5));
    await expect(b).resolves.toBe(99);
    await expect(a).resolves.toBe(5);
  });

  it('rejects when the host reports an error', async () => {
    const p = client.request('list_files');
    const id = (transport.outbound[0] as { id: number }).id;
    transport.push({ type: 'CommandResponse', id, ok: false, error: 'mutates host library' });
    await expect(p).rejects.toThrow('mutates host library');
  });

  it('listFiles sends the right command name', async () => {
    void client.listFiles({ query: 'beach', limit: 50 });
    const sent = transport.outbound[0] as Extract<GuestOutbound, { type: 'CommandRequest' }>;
    expect(sent.name).toBe('list_files');
    expect(sent.payload.limit).toBe(50);
    expect(sent.payload.query).toBe('beach');
  });

  it('snake-cases camelCase payload keys before sending over WebRTC', async () => {
    // The host's `dispatch` reads snake_case (album_id, favorites_only …). A
    // camelCase key passed verbatim would silently default / fail, so the guest
    // transport must translate it like the webHost transport does.
    void client.listFiles({ favoritesOnly: true, albumId: 'alb1' });
    void client.request('set_favorites', { ids: ['a'], favorite: true });
    const files = transport.outbound[0] as Extract<GuestOutbound, { type: 'CommandRequest' }>;
    expect(files.payload).toMatchObject({ favorites_only: true, album_id: 'alb1' });
    expect(files.payload).not.toHaveProperty('albumId');
    expect(files.payload).not.toHaveProperty('favoritesOnly');
    const favs = transport.outbound[1] as Extract<GuestOutbound, { type: 'CommandRequest' }>;
    expect(favs.payload).toEqual({ ids: ['a'], favorite: true });
  });

  it('assembles media chunks into a cached blob URL and notifies onMedia', async () => {
    const onMedia = vi.fn();
    client = new GuestClient(transport, { onMedia });
    client.fetchOriginal('p1');
    expect(transport.outbound[0]).toEqual({
      type: 'FetchMediaRequest',
      id: 'p1',
      thumbnail: false,
    });

    transport.push({ type: 'FileHeader', id: 'p1', filename: 'pic.jpg', size: 2 });
    transport.push({ type: 'FileChunk', id: 'p1', index: 0, data: 'qg==' }); // [0xaa]
    transport.push({ type: 'FileChunk', id: 'p1', index: 1, data: 'uw==' }); // [0xbb]
    transport.push({ type: 'FileEnd', id: 'p1' });

    expect(onMedia).toHaveBeenCalledTimes(1);
    const [id, key, url] = onMedia.mock.calls[0];
    expect(id).toBe('p1');
    expect(key).toBe('original:p1');
    expect(typeof url).toBe('string');
    expect(client.cachedUrl('original:p1')).toBe(url);
  });

  it('handles inline ViewMedia blobs', async () => {
    const onMedia = vi.fn();
    client = new GuestClient(transport, { onMedia });
    // base64 for a 2-byte payload
    const data = btoa(String.fromCharCode(1, 2));
    transport.push({ type: 'ViewMedia', id: 'p1', mime: 'image/jpeg', data });
    expect(onMedia).toHaveBeenCalledTimes(1);
    const [, key, url] = onMedia.mock.calls[0];
    expect(key).toBe('thumb:p1');
    expect(client.cachedUrl('thumb:p1')).toBe(url);
  });

  it('does not let an original and thumbnail for the SAME id clobber each other', async () => {
    const onMedia = vi.fn();
    client = new GuestClient(transport, { onMedia });
    // Fetch thumbnail first, then original, for the same photo id.
    const thumb = client.fetchThumb('p1');
    const orig = client.fetchOriginal('p1');
    // Only the original registers a FileAssembler (thumbnails are ViewMedia).
    const thumbReq = transport.outbound[0] as Extract<GuestOutbound, { type: 'FetchMediaRequest' }>;
    const origReq = transport.outbound[1] as Extract<GuestOutbound, { type: 'FetchMediaRequest' }>;
    expect(thumbReq.thumbnail).toBe(true);
    expect(origReq.thumbnail).toBe(false);

    // Thumbnail arrives inline as ViewMedia.
    const data = btoa(String.fromCharCode(9, 9));
    transport.push({ type: 'ViewMedia', id: 'p1', mime: 'image/jpeg', data });

    // Original streams via FileHeader/Chunk/End.
    transport.push({ type: 'FileHeader', id: 'p1', filename: 'full.jpg', size: 2 });
    transport.push({ type: 'FileChunk', id: 'p1', index: 0, data: 'AQ==' }); // [0x01]
    transport.push({ type: 'FileChunk', id: 'p1', index: 1, data: 'Ag==' }); // [0x02]
    transport.push({ type: 'FileEnd', id: 'p1' });

    await expect(orig).resolves.toBeTruthy();
    await expect(thumb).resolves.toBeTruthy();

    // Each kind resolves to a distinct cached URL (no cross-contamination).
    const thumbUrl = client.cachedUrl('thumb:p1');
    const origUrl = client.cachedUrl('original:p1');
    expect(thumbUrl).toBeTruthy();
    expect(origUrl).toBeTruthy();
    expect(thumbUrl).not.toBe(origUrl);
    // The original's promise resolved to the *original* URL, not the thumb's.
    expect(await orig).toBe(origUrl);
  });

  it('rejects pending requests when the session closes', async () => {
    const p = client.request('count_trash');
    client.close();
    await expect(p).rejects.toThrow('Session ended');
  });
});
