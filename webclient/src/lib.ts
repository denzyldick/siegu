/**
 * Pure utility functions extracted from main.ts for testability.
 * These have no side effects and no DOM dependencies.
 */

export interface ViewPhoto {
  id: string;
  location: string;
  created: string;
  caption: string | null;
}

export type SyncMsg =
  | { type: 'ViewOnlyManifest'; photos: ViewPhoto[]; more: boolean }
  | { type: 'ViewMedia'; id: string; mime: string; data: string }
  | {
      type: 'FileHeader';
      id: string;
      filename: string;
      size: number;
      checksum: string;
    }
  | { type: 'FileChunk'; id: string; index: number; data: number[] }
  | { type: 'FileEnd'; id: string; checksum: string }
  | { type: string; [k: string]: unknown };

/** Parse "#CODE.TOKEN[.ALBUM_ID][.MIN|.once]" from the URL fragment. */
export function parseHash(
  hash: string,
): { code: string; token: string; albumId?: string; minutes?: number; oneTime?: boolean } | null {
  const raw = decodeURIComponent(hash.replace(/^#/, ''));
  const parts = raw.split('.');
  if (parts.length < 2) return null;
  const [code, token, albumId, flag] = parts;
  if (!code || !token || code.includes('/') || token.includes('/'))
    return null;
  const result: { code: string; token: string; albumId?: string; minutes?: number; oneTime?: boolean } = {
    code,
    token,
    albumId: albumId || undefined,
  };
  if (flag && /^\d+$/.test(flag)) {
    const mins = parseInt(flag, 10);
    if (mins > 0) result.minutes = mins;
  } else if (flag === 'once') {
    result.oneTime = true;
  }
  return result;
}

/** Infer MIME type from a filename extension. */
export function inferMime(filename: string): string {
  const name = filename.toLowerCase();
  if (/\.(mp4|mov|m4v)$/.test(name)) return 'video/mp4';
  if (/\.webm$/.test(name)) return 'video/webm';
  return 'image/jpeg';
}

/**
 * Reassemble chunked file data into a single byte array.
 * Returns the assembled bytes, or null if no chunks exist.
 */
export function assembleChunks(
  chunks: Map<number, number[]>,
): Uint8Array | null {
  if (chunks.size === 0) return null;
  const indexes = [...chunks.keys()].sort((a, b) => a - b);
  let len = 0;
  for (const i of indexes) len += chunks.get(i)!.length;
  const bytes = new Uint8Array(len);
  let offset = 0;
  for (const i of indexes) {
    bytes.set(chunks.get(i)!, offset);
    offset += chunks.get(i)!.length;
  }
  return bytes;
}
