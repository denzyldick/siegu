/**
 * Runtime UI-mode detection (#24, Phase 1).
 *
 * One binary / Vue build serves three runtime modes:
 *   - `desktop` (BackendMode 'tauri') — native Tauri app, unchanged.
 *   - `webHost` (Mode A) — the browser is the OWNER of a library mounted on a
 *     `siegu web` instance (e.g. the VPS docker deploy). Detected by probing the
 *     host's `/session` endpoint, which the Rust host serves unconditionally.
 *   - `guest` (Mode B) — the browser PAIRS by code + token with a remote/desktop
 *     Siegu and streams media over WebRTC (web.whatsapp.com model). Detected by a
 *     `#CODE.TOKEN` hash in the URL fragment.
 *   - `onboarding` — neither (fresh / landing / no library yet).
 *
 * Only the `webHost` probe touches the network; it is non-blocking and bounded by
 * an AbortController timeout.
 */
import { isTauri } from '@/services/tauri';
import { parseHash, type GuestSession } from '@/services/backend/protocol';
import type { RuntimeMode } from '@/services/backend/interface';

export interface DetectedMode {
  mode: RuntimeMode;
  /** Present only when mode === 'guest'. */
  session?: GuestSession;
}

const WEBHOST_PROBE_TIMEOUT_MS = 1500;

/**
 * Probe the `webHost` `/session` endpoint (web.rs `serve_static`). Resolves true
 * when the host answers `{ code }` with a non-empty code. Bounded + non-blocking.
 */
export async function isWebHost(): Promise<boolean> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), WEBHOST_PROBE_TIMEOUT_MS);
  try {
    const res = await fetch('/session', { signal: controller.signal });
    if (!res.ok) return false;
    const body = (await res.json()) as { code?: string };
    return typeof body.code === 'string' && body.code.length > 0;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}

/** Parse a `#CODE.TOKEN[.ALBUM_ID]` session from the URL fragment, if present. */
export function guestSessionFromHash(hash: string = window.location.hash): GuestSession | null {
  return parseHash(hash);
}

/** Resolve the runtime mode, in priority order (see module docs). */
export async function detectMode(): Promise<DetectedMode> {
  if (isTauri) {
    return { mode: 'tauri' };
  }
  if (await isWebHost()) {
    return { mode: 'webHost' };
  }
  const session = guestSessionFromHash();
  if (session) {
    return { mode: 'guest', session };
  }
  return { mode: 'onboarding' };
}
