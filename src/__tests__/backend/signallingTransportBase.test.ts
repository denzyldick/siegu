import { describe, it, expect, vi } from 'vitest';
import { bootGuest } from '@/services/backend/bootGuest';
import { createPeerTransport } from '@/services/backend/peer';
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
