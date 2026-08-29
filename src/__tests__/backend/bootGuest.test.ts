import { describe, it, expect, beforeEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { bootGuest, GuestClient } from '@/services/backend';
import { useRuntimeStore } from '@/stores/runtime';
import type { PeerTransport } from '@/services/backend/peer';
import type { GuestOutbound } from '@/services/backend/protocol';

/**
 * Fake peer transport that never touches the network but lets the test emit
 * lifecycle events (open/close) as the real host would.
 */
class FakeTransport implements PeerTransport {
  outbound: GuestOutbound[] = [];
  private openHandlers: Array<() => void> = [];
  private closeHandlers: Array<() => void> = [];
  private opened = false;

  send(msg: GuestOutbound): void {
    this.outbound.push(msg);
  }
  onMessage(): void {}
  onOpen(h: () => void): void {
    this.openHandlers.push(h);
    if (this.opened) h();
  }
  onClose(h: () => void): void {
    this.closeHandlers.push(h);
  }
  isOpen(): boolean {
    return this.opened;
  }
  close(): void {
    this.opened = false;
    this.closeHandlers.forEach((h) => h());
  }
  emitOpen(): void {
    this.opened = true;
    this.openHandlers.forEach((h) => h());
  }
}

const SESSION = { code: 'ABC123', token: 'tok' };

describe('bootGuest', () => {
  it('builds a GuestClient over an injected transport', () => {
    const transport = new FakeTransport();
    const { client, transport: t } = bootGuest(SESSION, {}, transport);
    expect(client).toBeInstanceOf(GuestClient);
    expect(t).toBe(transport);
  });
});

describe('runtime store: connectGuest (Mode B)', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('registers the client, reports connected on open, and switches backend to guest', async () => {
    const transport = new FakeTransport();
    const store = useRuntimeStore();
    // Mode A/B resolution normally happens in initRuntime(); set it directly for
    // an isolated test of the connection flow.
    store.mode = 'guest';
    store.session = SESSION;

    await store.connectGuest(SESSION, {}, transport);
    expect(store.guestClient).not.toBeNull();
    expect(store.guestConnection).toBe('connecting');

    transport.emitOpen();
    expect(store.guestConnection).toBe('connected');
    expect(store.isGuestConnected).toBe(true);

    const backend = store.backend;
    expect(backend.listFiles).toBeInstanceOf(Function);
    expect(backend.mediaUrl).toBeInstanceOf(Function);
  });

  it('throws when no guest session is provided', async () => {
    const store = useRuntimeStore();
    store.mode = 'guest';
    store.session = undefined;

    await expect(store.connectGuest(undefined)).rejects.toThrow(/No guest session/);
  });
});
