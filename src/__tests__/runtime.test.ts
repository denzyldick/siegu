import { describe, it, expect, beforeEach } from 'vitest';
import { detectMode, guestSessionFromHash } from '@/services/runtime';

describe('runtime mode detection', () => {
  beforeEach(() => {
    window.location.hash = '';
  });

  it('parses a #code.token guest session from the hash', () => {
    expect(guestSessionFromHash('#ABCD1234.abcT0ken')).toEqual({
      code: 'ABCD1234',
      token: 'abcT0ken',
    });
  });

  it('parses an album-scoped guest session', () => {
    expect(guestSessionFromHash('#CODE.TOKEN.42')).toEqual({
      code: 'CODE',
      token: 'TOKEN',
      albumId: '42',
    });
  });

  it('returns null for a hash without code.token', () => {
    expect(guestSessionFromHash('#not-a-session')).toBeNull();
    expect(guestSessionFromHash('')).toBeNull();
  });

  it('detects guest mode when a session hash is present', async () => {
    window.location.hash = '#ABC123456789.abcdef';
    const result = await detectMode();
    // In happy-dom (no Tauri, no /session route) the hash is the only trigger.
    expect(result.mode).toBe('guest');
    expect(result.session?.code).toBe('ABC123456789');
  });
});
