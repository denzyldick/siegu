import { describe, it, expect } from 'vitest';
import { parseHash } from '../lib';

describe('parseHash', () => {
  it('parses code and token', () => {
    expect(parseHash('#ABC123.XYZ789')).toEqual({
      code: 'ABC123',
      token: 'XYZ789',
      albumId: undefined,
    });
  });

  it('parses code, token, and albumId', () => {
    expect(parseHash('#CODE.TOKEN.ALBUM1')).toEqual({
      code: 'CODE',
      token: 'TOKEN',
      albumId: 'ALBUM1',
    });
  });

  it('returns null for single part', () => {
    expect(parseHash('#only_one')).toBeNull();
  });

  it('returns null for empty hash', () => {
    expect(parseHash('#')).toBeNull();
  });

  it('returns null for empty code', () => {
    expect(parseHash('#.TOKEN')).toBeNull();
  });

  it('returns null for empty token', () => {
    expect(parseHash('#CODE.')).toBeNull();
  });

  it('returns null for slash in code', () => {
    expect(parseHash('#A/B.TOKEN')).toBeNull();
  });

  it('returns null for slash in token', () => {
    expect(parseHash('#CODE.T/O')).toBeNull();
  });

  it('ignores extra parts after albumId', () => {
    const result = parseHash('#A.B.C.D.E');
    expect(result).toEqual({ code: 'A', token: 'B', albumId: 'C' });
  });

  it('handles URL-encoded characters in token', () => {
    expect(parseHash('#CODE.TOKEN%20WITH%20SPACE')).toEqual({
      code: 'CODE',
      token: 'TOKEN WITH SPACE',
      albumId: undefined,
    });
  });
});
