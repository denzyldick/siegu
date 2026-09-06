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
  | { type: 'FileChunk'; id: string; index: number; data: string }
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

/** Decode a base64 chunk payload (mirrors src/services/backend/protocol.ts). */
export function b64ToBytes(b64: string): Uint8Array {
  const raw = atob(b64);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return bytes;
}

/**
 * Reassemble chunked file data into a single byte array.
 * Returns the assembled bytes, or null if no chunks exist.
 */
export function assembleChunks(
  chunks: Map<number, Uint8Array>,
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

/** ICE config the host stamps into the served page when its built-in TURN
 * relay is enabled (`window.sieguTurnConfig`), mirroring the desktop guest's
 * `TURNConfig`. */
export interface SieguTurnConfig {
  url?: string | string[];
  username?: string;
  credential?: string;
}

/** Read the relay config the host injected into this page, if any. */
export function readSieguTurnConfig(
  win?: { sieguTurnConfig?: SieguTurnConfig },
): SieguTurnConfig | undefined {
  const root =
    win ?? (globalThis as unknown as { sieguTurnConfig?: SieguTurnConfig });
  const cfg = root.sieguTurnConfig;
  if (!cfg || !cfg.username || !cfg.credential) return undefined;
  const urls = Array.isArray(cfg.url)
    ? cfg.url
    : (cfg.url ?? '')
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
  if (urls.length === 0) return undefined;
  return { url: urls, username: cfg.username, credential: cfg.credential };
}

/** ICE servers for this guest: public STUN plus the host's relay when present. */
export function turnIceServers(turn?: SieguTurnConfig): RTCIceServer[] {
  const servers: RTCIceServer[] = [{ urls: 'stun:stun.l.google.com:19302' }];
  const cfg = turn ?? readSieguTurnConfig();
  if (cfg?.url && cfg.username) {
    servers.push({
      urls: Array.isArray(cfg.url) ? cfg.url : [cfg.url],
      username: cfg.username,
      credential: cfg.credential ?? '',
    });
  }
  return servers;
}
