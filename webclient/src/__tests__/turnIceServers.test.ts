import { describe, expect, it } from 'vitest';
import { readSieguTurnConfig, turnIceServers, type SieguTurnConfig } from '../lib';

describe('readSieguTurnConfig', () => {
  it('returns undefined when the host injected nothing', () => {
    expect(readSieguTurnConfig({})).toBeUndefined();
    expect(readSieguTurnConfig({ sieguTurnConfig: { url: 'turn:1.2.3.4:3478' } })).toBeUndefined();
  });

  it('normalizes the comma-separated URL string from the host', () => {
    const cfg = readSieguTurnConfig({
      sieguTurnConfig: {
        url: 'turn:192.168.1.5:51820, turn:127.0.0.1:51820',
        username: 'alice',
        credential: 'secret',
      },
    });
    expect(cfg).toEqual({
      url: ['turn:192.168.1.5:51820', 'turn:127.0.0.1:51820'],
      username: 'alice',
      credential: 'secret',
    });
  });

  it('passes through an array url verbatim', () => {
    const cfg = readSieguTurnConfig({
      sieguTurnConfig: { url: ['turn:relay.siegu.io:3478'], username: 'u', credential: 'p' },
    });
    expect(cfg?.url).toEqual(['turn:relay.siegu.io:3478']);
  });
});

describe('turnIceServers', () => {
  it('keeps the google STUN default when no relay is configured', () => {
    const servers = turnIceServers(undefined);
    expect(servers).toEqual([{ urls: 'stun:stun.l.google.com:19302' }]);
  });

  it('appends the relay with credentials when configured', () => {
    const turn: SieguTurnConfig = { url: 'turn:relay.siegu.io:3478', username: 'u', credential: 'p' };
    const servers = turnIceServers(turn);
    expect(servers).toHaveLength(2);
    expect(servers[1]).toEqual({
      urls: ['turn:relay.siegu.io:3478'],
      username: 'u',
      credential: 'p',
    });
  });
});