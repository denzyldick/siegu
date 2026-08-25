import { describe, it, expect } from 'vitest';
import { parseHash } from '../lib';

// Integration-style tests: simulate real URL patterns users would encounter

describe('real-world URL patterns', () => {
  it('LAN URL from siegu web', () => {
    // e.g. http://192.168.1.5:8787/#ABC123.XYZ789
    const hash = '#ABC123.XYZ789';
    const session = parseHash(hash);
    expect(session).toEqual({
      code: 'ABC123',
      token: 'XYZ789',
      albumId: undefined,
    });
  });

  it('album share URL', () => {
    // e.g. http://10.0.0.1:8787/#ROOM_ID.TOKEN.ALBUM_42
    const hash = '#room123.tok456.album42';
    const session = parseHash(hash);
    expect(session).toEqual({
      code: 'room123',
      token: 'tok456',
      albumId: 'album42',
    });
  });

  it('URL with special characters in token', () => {
    const hash = '#CODE.abc-123_def.ALBUM';
    const session = parseHash(hash);
    expect(session).toEqual({
      code: 'CODE',
      token: 'abc-123_def',
      albumId: 'ALBUM',
    });
  });

  it('empty fragment shows no session', () => {
    expect(parseHash('')).toBeNull();
  });

  it('hash with only # shows no session', () => {
    expect(parseHash('#')).toBeNull();
  });
});
