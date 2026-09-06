import { describe, it, expect, vi } from 'vitest';
import { bootGuest } from '@/services/backend/bootGuest';
import { buildIceServers, createPeerTransport } from '@/services/backend/peer';
import type { GuestSession } from '@/services/backend/protocol';

/**
 * Phase 4: the `signalingBase` a guest supplies must reach `createPeerTransport`
 * (and from there the signaler's `/ws`), so a Mode B guest can pair against a
 * hosted `wss://` relay rather than only the serving host's own bridge.
 *
 * `createPeerTransport` is mocked so no real WebSocket/RTCPeerConnection is
 * constructed; we assert the base string is forwarded verbatim.
 */

vi.mock('@/services/backend/peer', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/services/backend/peer')>();
  return { ...actual, createPeerTransport: vi.fn() };
});

const SESSION: GuestSession = { code: 'ABC123', token: 'tok' };

describe('bootGuest signalling base wiring', () => {
  it('forwards an explicit signalingBase to createPeerTransport', () => {
    vi.mocked(createPeerTransport).mockReturnValue({
      send: () => {},
      onMessage: () => {},
      onOpen: () => {},
      onClose: () => {},
      isOpen: () => false,
      close: () => {},
    });

    bootGuest(SESSION, {}, undefined, 'relay.siegu.io');

    expect(vi.mocked(createPeerTransport)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(createPeerTransport).mock.calls[0][0]).toBe('relay.siegu.io');
    expect(vi.mocked(createPeerTransport).mock.calls[0][1]).toEqual(SESSION);
  });

  it('still builds a client when a transport override is supplied (no network)', () => {
    vi.mocked(createPeerTransport).mockClear();
    const fake = {
      send: () => {},
      onMessage: () => {},
      onOpen: () => {},
      onClose: () => {},
      isOpen: () => false,
      close: () => {},
    };
    const { client } = bootGuest(SESSION, {}, fake, 'relay.siegu.io');
    expect(client).toBeDefined();
    // Override short-circuits the real builder.
    expect(vi.mocked(createPeerTransport)).not.toHaveBeenCalled();
  });
});

describe('buildIceServers', () => {
  it('defaults to the public STUN server alone', () => {
    const servers = buildIceServers();
    expect(servers).toHaveLength(1);
    expect(servers[0].urls).toContain('stun:stun.l.google.com:19302');
  });

  it('appends a TURN server when configured, preserving the STUN default', () => {
    const servers = buildIceServers({
      urls: ['turn:turn.siegu.io:3478'],
      username: 'ghost',
      credential: 'hunter2',
    });
    expect(servers).toHaveLength(2);
    expect(servers[0].urls).toContain('stun:stun.l.google.com:19302');
    expect(servers[1].urls).toEqual(['turn:turn.siegu.io:3478']);
    expect(servers[1].username).toBe('ghost');
    expect(servers[1].credential).toBe('hunter2');
  });

  it('ignores an empty TURN URL list', () => {
    const servers = buildIceServers({ urls: [] });
    expect(servers).toHaveLength(1);
  });
});
