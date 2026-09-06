import { describe, it, expect } from 'vitest';
import { assembleChunks } from '../lib';

describe('assembleChunks', () => {
  it('returns null for empty map', () => {
    expect(assembleChunks(new Map())).toBeNull();
  });

  it('assembles single chunk', () => {
    const chunks = new Map([[0, new Uint8Array([1, 2, 3])]]);
    const result = assembleChunks(chunks);
    expect(result).toEqual(new Uint8Array([1, 2, 3]));
  });

  it('assembles multiple chunks in order', () => {
    const chunks = new Map<number, Uint8Array>([
      [2, new Uint8Array([7, 8])],
      [0, new Uint8Array([1, 2])],
      [1, new Uint8Array([3, 4, 5, 6])],
    ]);
    const result = assembleChunks(chunks);
    expect(result).toEqual(new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]));
  });

  it('handles non-contiguous indexes', () => {
    const chunks = new Map<number, Uint8Array>([
      [0, new Uint8Array([10, 20])],
      [5, new Uint8Array([30])],
    ]);
    const result = assembleChunks(chunks);
    // Index 0: [10,20], index 5: [30] — sorted by key
    expect(result!.length).toBe(3);
    expect(result![0]).toBe(10);
    expect(result![1]).toBe(20);
    expect(result![2]).toBe(30);
  });

  it('handles empty chunks in between', () => {
    const chunks = new Map<number, Uint8Array>([
      [0, new Uint8Array([1])],
      [1, new Uint8Array(0)],
      [2, new Uint8Array([2])],
    ]);
    const result = assembleChunks(chunks);
    expect(result).toEqual(new Uint8Array([1, 2]));
  });
});