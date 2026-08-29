import { describe, it, expect, vi, afterEach } from 'vitest';
import { webHostBackend, createBackend, mediaCacheKey } from '@/services/backend';
import type { MediaItem } from '@/types/media';

function mockFetchOnce(body: unknown, status = 200): void {
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue(
      new Response(JSON.stringify(body), {
        status,
        headers: { 'content-type': 'application/json' },
      }),
    ),
  );
}

function mockFetchNetworkError(): void {
  vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')));
}

describe('webHostBackend (Mode A over HTTP/RPC)', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('is selected by createBackend for the webHost mode', () => {
    const backend = createBackend('webHost');
    expect(backend.listFiles).toBeInstanceOf(Function);
    expect(backend.mediaUrl).toBeInstanceOf(Function);
  });

  it('posts list_files to /rpc and resolves the result', async () => {
    const item = {
      id: 1,
      path: '/p.jpg',
      filename: 'p.jpg',
      type: 'image',
    } as unknown as MediaItem;
    mockFetchOnce({ ok: true, result: [item] });
    const backend = webHostBackend();
    const items = await backend.listFiles({ query: 'sunset' });
    expect(items).toEqual([item]);
    const fetchMock = vi.mocked(fetch);
    expect(fetchMock).toHaveBeenCalledWith(
      '/rpc',
      expect.objectContaining({
        method: 'POST',
        body: expect.stringContaining('"name":"list_files"'),
      }),
    );
  });

  it('rejects when the host /rpc endpoint is unreachable', async () => {
    mockFetchNetworkError();
    const backend = webHostBackend();
    await expect(backend.searchFacets()).rejects.toThrow(/not reachable/);
  });

  it('rejects on a non-ok RPC error payload', async () => {
    mockFetchOnce({ ok: false, error: 'read-only mode' });
    const backend = webHostBackend();
    await expect(backend.trashPhoto(5)).rejects.toThrow(/read-only mode/);
  });

  it('resolves and caches media URLs by id + kind', async () => {
    const backend = webHostBackend();
    expect(await backend.mediaUrl(7, 'thumb')).toBe('/thumb/7');
    expect(await backend.mediaUrl('abc', 'original')).toBe('/media/abc');
    // cached sync lookup
    expect(backend.cachedMediaUrl(7, 'thumb')).toBe('/thumb/7');
    expect(backend.cachedMediaUrl(7, 'original')).toBeUndefined();
    void mediaCacheKey; // parity with other backends
  });

  it('sends the webHost token as a Bearer header on /rpc', async () => {
    mockFetchOnce({ ok: true, result: [] });
    const backend = webHostBackend('secret-token');
    await backend.listFiles({});
    const fetchMock = vi.mocked(fetch);
    expect(fetchMock).toHaveBeenCalledWith(
      '/rpc',
      expect.objectContaining({
        headers: { 'content-type': 'application/json', authorization: 'Bearer secret-token' },
      }),
    );
  });

  it('appends ?token= to thumb and media URLs when a webHost token is present', async () => {
    const backend = webHostBackend('secret-token');
    expect(await backend.mediaUrl(7, 'thumb')).toBe('/thumb/7?token=secret-token');
    expect(await backend.mediaUrl('abc', 'original')).toBe('/media/abc?token=secret-token');
    expect(backend.cachedMediaUrl(7, 'thumb')).toBe('/thumb/7?token=secret-token');
  });
});
