import { describe, it, expect, vi } from 'vitest';
import { GuestClient, guestBackend, createBackend, mediaCacheKey } from '@/services/backend';
import type { Backend, GuestInbound, GuestOutbound } from '@/services/backend';

class FakeTransport {
  outbound: GuestOutbound[] = [];
  private msgHandler: ((msg: GuestInbound) => void) | null = null;
  isOpenValue = true;
  send(msg: GuestOutbound): void {
    this.outbound.push(msg);
  }
  onMessage(h: (msg: GuestInbound) => void): void {
    this.msgHandler = h;
  }
  onOpen(): void {}
  onClose(): void {}
  isOpen(): boolean {
    return this.isOpenValue;
  }
  close(): void {}
  reply(name: string, result: unknown): void {
    const req = [...this.outbound]
      .reverse()
      .find(
        (m): m is Extract<GuestOutbound, { type: 'CommandRequest' }> =>
          m.type === 'CommandRequest' && m.name === name,
      );
    this.msgHandler?.({ type: 'CommandResponse', id: req!.id, ok: true, result });
  }
}

function makeRaw(): { raw: GuestClient; backend: ReturnType<typeof guestBackend> } {
  const transport = new FakeTransport();
  const raw = new GuestClient(transport);
  const backend = guestBackend(raw);
  return { raw, backend };
}

describe('guestBackend', () => {
  describe('data delegation', () => {
    const cases: Array<[keyof Backend & keyof GuestClient, ...unknown[]]> = [
      ['listFiles', {}],
      ['getPhotoById', 5],
      ['searchFacets'],
      ['countTrash'],
      ['listTrash', 50],
      ['toggleFavorite', 5],
      ['trashPhoto', 'a'],
      ['restorePhoto', 'a'],
      ['emptyTrash'],
    ];

    it.each(cases)(
      'delegates %s',
      async (method: keyof Backend & keyof GuestClient, ...args: unknown[]) => {
        const { raw, backend } = makeRaw();
        const spy = vi.spyOn(raw, method).mockResolvedValue(null as never);
        await (backend[method] as (...a: unknown[]) => Promise<unknown>)(...args);
        expect(spy).toHaveBeenCalledWith(...args);
      },
    );
  });

  it('maps mediaUrl thumb to fetchThumb', async () => {
    const { raw, backend } = makeRaw();
    const spy = vi.spyOn(raw, 'fetchThumb').mockResolvedValue('blob:thumb');
    await expect(backend.mediaUrl(3, 'thumb')).resolves.toBe('blob:thumb');
    expect(spy).toHaveBeenCalledWith(3);
  });

  it('maps mediaUrl original to fetchOriginal', async () => {
    const { raw, backend } = makeRaw();
    const spy = vi.spyOn(raw, 'fetchOriginal').mockResolvedValue('blob:orig');
    await expect(backend.mediaUrl(3, 'original')).resolves.toBe('blob:orig');
    expect(spy).toHaveBeenCalledWith(3);
  });

  it('exposes cached media URLs by cache key', () => {
    const { raw, backend } = makeRaw();
    vi.spyOn(raw, 'cachedUrl').mockReturnValue('blob:cached');
    expect(backend.cachedMediaUrl(7, 'thumb')).toBe('blob:cached');
    expect(raw.cachedUrl).toHaveBeenCalledWith(mediaCacheKey(7, 'thumb'));
  });

  it('close() forwards to the guest client', () => {
    const { raw, backend } = makeRaw();
    const spy = vi.spyOn(raw, 'close').mockImplementation(() => {});
    backend.close();
    expect(spy).toHaveBeenCalled();
  });
});

describe('createBackend', () => {
  it('returns a tauri backend by default', () => {
    const b = createBackend('tauri');
    expect(typeof b.mediaUrl).toBe('function');
    expect(typeof b.listFiles).toBe('function');
  });

  it('returns a guest backend when given a client', () => {
    const transport = new FakeTransport();
    const client = new GuestClient(transport);
    const b = createBackend('guest', client);
    expect(typeof b.mediaUrl).toBe('function');
  });
});
