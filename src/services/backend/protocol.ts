/**
 * Pure, side-effect-free types + helpers for the WebRTC RPC guest backend (#19).
 * No DOM / WebRTC / network dependencies — everything here is unit-testable.
 *
 * Message shapes mirror `crates/siegu-core/src/mesh.rs` and `rpc.rs`:
 *   - CommandRequest  { id: u64, name, payload }
 *   - CommandResponse { id: u64, ok, result?, error? }
 *   - File media transfer (FileHeader / FileChunk / FileEnd)
 */

/** What a browser guest sends over the data channel. */
export type GuestOutbound =
  | { type: 'CommandRequest'; id: number; name: string; payload: Record<string, unknown> }
  | { type: 'FetchMediaRequest'; id: number | string; thumbnail: boolean }
  | { type: 'EnterViewOnly' }
  | { type: 'EnterAlbumShare'; album_id: string };

/** What the host sends back over the data channel. */
export type GuestInbound =
  | { type: 'CommandResponse'; id: number; ok: boolean; result?: unknown; error?: string }
  | { type: 'ViewMedia'; id: number | string; mime: string; data: string }
  | {
      type: 'FileHeader';
      id: number | string;
      filename: string;
      size: number;
      checksum?: string;
    }
  | { type: 'FileChunk'; id: number | string; index: number; data: number[] }
  | { type: 'FileEnd'; id: number | string; checksum?: string };
// Unknown inbound frames fall through to the `default` case at runtime; there
// is intentionally no catch-all union member so TypeScript can narrow the
// known shapes above.

/** Parse "#CODE.TOKEN[.ALBUM_ID][.MIN|.once]" from the URL fragment. */
export interface GuestSession {
  code: string;
  token: string;
  albumId?: string;
  /** Present when the link carries a `.MIN` duration flag. */
  minutes?: number;
  /** Present when the link carries a `.once` (one-time view) flag. */
  oneTime?: boolean;
}

export function parseHash(hash: string): GuestSession | null {
  const raw = decodeURIComponent(hash.replace(/^#/, ''));
  const parts = raw.split('.');
  if (parts.length < 2) return null;
  const [code, token, albumId, flag] = parts;
  if (!code || !token || code.includes('/') || token.includes('/')) return null;
  const result: GuestSession = {
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

/**
 * Decide whether the WebRTC data channel can accept the next queued frame, and
 * if so, pop it from the outbox. This is the pure, testable core of the guest
 * transport's SCTP backpressure handling: it only lets a frame through when
 * the channel is open AND the buffered amount is at or below the ceiling, so a
 * busy channel parks frames on the outbox queue instead of dropping them.
 */
export function takeNextOutbound(
  outbox: GuestOutbound[],
  channelOpen: boolean,
  bufferedAmount: number,
  threshold = 1_000_000,
): GuestOutbound | null {
  if (!channelOpen) return null;
  if (bufferedAmount > threshold) return null;
  return outbox.shift() ?? null;
}

/** Infer a MIME type from a filename extension (mirrors webclient/lib.ts). */
export function inferMime(filename: string): string {
  const name = filename.toLowerCase();
  if (/\.(mp4|mov|m4v)$/.test(name)) return 'video/mp4';
  if (/\.webm$/.test(name)) return 'video/webm';
  return 'image/jpeg';
}

/**
 * Reassemble a set of file chunks into a single byte array.
 * Returns null when no chunks have been received.
 */
export function assembleChunks(chunks: Map<number, number[]>): Uint8Array | null {
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

/**
 * Tracks an in-flight media transfer keyed by media id: buffers FileChunk
 * payloads, then assembles a Blob on FileEnd. Pure state machine, testable.
 */
export class FileAssembler {
  private filename: string | undefined;
  private chunks = new Map<number, number[]>();

  constructor(
    public readonly id: number | string,
    private readonly onDone: (blob: Blob, filename: string, mime: string) => void,
  ) {}

  header(filename: string): void {
    this.filename = filename;
  }

  chunk(index: number, data: number[]): void {
    this.chunks.set(index, data);
  }

  end(): void {
    if (this.chunks.size === 0 || !this.filename) return;
    const bytes = assembleChunks(this.chunks);
    this.chunks.clear();
    if (!bytes) return;
    const mime = inferMime(this.filename);
    const blob = new Blob([bytes], { type: mime });
    this.onDone(blob, this.filename, mime);
  }

  reset(): void {
    this.chunks.clear();
  }
}
