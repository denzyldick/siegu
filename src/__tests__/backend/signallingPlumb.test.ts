import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { resolveSignalingBase } from '@/services/signalling';

/**
 * Phase 4: hosted `wss://` signalling plumbing.
 *
 * A Mode B guest must be able to pair by code + token against a remote
 * signaler, not just the `/ws` bridge of the CLI host that served the page.
 * `resolveSignalingBase` picks where the guest connects; `bootGuest` hands that
 * base to the transport builder.
 */

describe('resolveSignalingBase', () => {
  const originalHost = window.location.host;
  const originalInjected = (window as { sieguSignalingHost?: string }).sieguSignalingHost;

  beforeEach(() => {
    delete (window as { sieguSignalingHost?: string }).sieguSignalingHost;
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, host: 'localhost' },
    });
  });

  afterEach(() => {
    if (originalInjected !== undefined) {
      (window as { sieguSignalingHost?: string }).sieguSignalingHost = originalInjected;
    }
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, host: originalHost },
    });
  });

  it('falls back to the serving origin when no remote signaler is configured', () => {
    delete (window as { sieguSignalingHost?: string }).sieguSignalingHost;
    expect(resolveSignalingBase()).toBe('localhost');
  });

  it('prefers a host-injected remote signaler over the serving origin', () => {
    (window as { sieguSignalingHost?: string }).sieguSignalingHost = 'relay.siegu.io';
    expect(resolveSignalingBase()).toBe('relay.siegu.io');
  });
});
