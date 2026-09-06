import { describe, it, expect, vi } from 'vitest';
import {
  parseHash,
  inferMime,
  assembleChunks,
  b64ToBytes,
  FileAssembler,
  takeNextOutbound,
} from '@/services/backend';
import type { GuestOutbound } from '@/services/backend';

function frame(type: string): GuestOutbound {
  return { type: 'CommandRequest', id: 1, name: type, payload: {} } as GuestOutbound;
}

describe('parseHash', () => {
  it('parses CODE.TOKEN', () => {
    expect(parseHash('#abc123.secTok')).toEqual({ code: 'abc123', token: 'secTok' });
  });

  it('parses CODE.TOKEN.ALBUM', () => {
    expect(parseHash('#abc123.secTok.alb-1')).toEqual({
      code: 'abc123',
      token: 'secTok',
      albumId: 'alb-1',
    });
  });

  it('decodes URL-encoded fragments', () => {
    expect(parseHash('#a%20b.c%20d')).toEqual({ code: 'a b', token: 'c d' });
  });

  it('parses a numeric duration flag as minutes', () => {
    expect(parseHash('#abc123.secTok.alb-1.30')).toEqual({
      code: 'abc123',
      token: 'secTok',
      albumId: 'alb-1',
      minutes: 30,
    });
  });

  it('parses the one-time flag', () => {
    expect(parseHash('#abc123.secTok.alb-1.once')).toEqual({
      code: 'abc123',
      token: 'secTok',
      albumId: 'alb-1',
      oneTime: true,
    });
  });

  it('ignores an unknown flag', () => {
    expect(parseHash('#abc123.secTok.alb-1.x')).toEqual({
      code: 'abc123',
      token: 'secTok',
      albumId: 'alb-1',
    });
  });

  it('returns null for malformed hashes', () => {
    expect(parseHash('')).toBeNull();
    expect(parseHash('#onlyCode')).toBeNull();
    expect(parseHash('#code/with/slash.token')).toBeNull();
    expect(parseHash('#no-hash')).toBeNull();
  });
});

describe('inferMime', () => {
  it('maps video extensions', () => {
    expect(inferMime('clip.mp4')).toBe('video/mp4');
    expect(inferMime('clip.mov')).toBe('video/mp4');
    expect(inferMime('clip.m4v')).toBe('video/mp4');
    expect(inferMime('clip.webm')).toBe('video/webm');
  });

  it('defaults images to jpeg', () => {
    expect(inferMime('photo.jpg')).toBe('image/jpeg');
    expect(inferMime('photo.png')).toBe('image/jpeg');
  });
});

describe('assembleChunks', () => {
  it('returns null with no chunks', () => {
    expect(assembleChunks(new Map())).toBeNull();
  });

  it('concatenates out-of-order chunks in index order', () => {
    const chunks = new Map<number, Uint8Array>([
      [1, new Uint8Array([2, 2])],
      [0, new Uint8Array([1, 1, 1])],
    ]);
    const bytes = assembleChunks(chunks);
    expect(bytes).not.toBeNull();
    expect([...bytes!]).toEqual([1, 1, 1, 2, 2]);
  });
});

describe('b64ToBytes', () => {
  it('decodes base64 payloads back to the original bytes', () => {
    expect([...b64ToBytes('AQI=')]).toEqual([1, 2]);
    expect([...b64ToBytes('AAA/AA==')]).toEqual([0, 0, 63, 0]);
  });
});

describe('FileAssembler', () => {
  it('assembles a Blob on end and calls onDone once', () => {
    const done = vi.fn();
    const asm = new FileAssembler('p1', (blob, filename, mime) => {
      expect(filename).toBe('pic.jpg');
      expect(mime).toBe('image/jpeg');
      expect(blob.size).toBeGreaterThan(0);
      done();
    });
    asm.header('pic.jpg');
    asm.chunk(0, 'AQI='); // [1, 2] base64
    asm.chunk(1, 'Aw=='); // [3] base64
    asm.end();
    expect(done).toHaveBeenCalledTimes(1);
  });

  it('does not call onDone when no chunks or no header', async () => {
    const done = vi.fn();
    const asm = new FileAssembler('p2', done);
    asm.end();
    asm.header('x.jpg');
    asm.end();
    await Promise.resolve();
    expect(done).not.toHaveBeenCalled();
  });
});

describe('takeNextOutbound (SCTP backpressure drain)', () => {
  it('parks frames while the channel is closed', () => {
    const outbox: GuestOutbound[] = [frame('a')];
    expect(takeNextOutbound(outbox, false, 0)).toBeNull();
    expect(outbox).toHaveLength(1);
  });

  it('parks frames while bufferedAmount exceeds the ceiling', () => {
    const outbox: GuestOutbound[] = [frame('a')];
    expect(takeNextOutbound(outbox, true, 2_000_000)).toBeNull();
    expect(outbox).toHaveLength(1);
  });

  it('releases frames once the channel is open and no longer backpressured', () => {
    // Simulate the buffered amount dropping back below the ceiling later in
    // the session (the "bufferedamountlow" re-flush scenario). The parked
    // frame must now drain so it is never stranded.
    const outbox: GuestOutbound[] = [frame('a')];
    expect(takeNextOutbound(outbox, true, 2_000_000)).toBeNull();
    const drained = takeNextOutbound(outbox, true, 100);
    expect(drained).toEqual(frame('a'));
    expect(outbox).toHaveLength(0);
  });

  it('returns null when the outbox is empty', () => {
    expect(takeNextOutbound([], true, 0)).toBeNull();
  });
});
