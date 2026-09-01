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
  /** Present only when mode === 'webHost': the host-issued data-plane token
   *  that authorizes `/rpc`, `/thumb`, `/media` (#28). */
  webHostToken?: string;
}

const WEBHOST_PROBE_TIMEOUT_MS = 1500;

/**
 * Probe the `webHost` `/session` endpoint (web.rs `serve_static`). Resolves true
 * when the host answers `{ code }` with a non-empty code, and caches the host's
 * data-plane `webToken` for later authorized data requests. Bounded + non-blocking.
 */
export async function isWebHost(): Promise<boolean> {
  const outcome = await probeWebHost();
  if (outcome.ok && outcome.webToken) lastWebHostToken = outcome.webToken;
  return outcome.ok;
}

/** Result of a single webHost `/session` probe. */
interface WebHostProbe {
  ok: boolean;
  webToken?: string;
}

let lastWebHostToken: string | undefined;

async function probeWebHost(): Promise<WebHostProbe> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), WEBHOST_PROBE_TIMEOUT_MS);
  try {
    const res = await fetch('/session', { signal: controller.signal });
    if (!res.ok) return { ok: false };
    const body = (await res.json()) as { code?: string; webToken?: string };
    const ok = typeof body.code === 'string' && body.code.length > 0;
    return { ok, webToken: typeof body.webToken === 'string' ? body.webToken : undefined };
  } catch {
    return { ok: false };
  } finally {
    clearTimeout(timer);
  }
}

/**
 * The data-plane token obtained from the last successful webHost probe, or
 * undefined. Used to authorize `/rpc`/`/thumb`/`/media` from the webHost mode.
 */
export function webHostToken(): string | undefined {
  return lastWebHostToken;
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
  // A `#CODE.TOKEN` fragment is unambiguously a shared-collection guest link.
  // Check it BEFORE the webHost probe so the same web build served by `siegu
  // web` (which also answers `/session`) still boots as a guest when a share
  // link is present, rather than being hijacked into webHost/owner mode.
  const session = guestSessionFromHash();
  if (session) {
    return { mode: 'guest', session };
  }
  if (await isWebHost()) {
    return { mode: 'webHost', webHostToken: webHostToken() };
  }
  return { mode: 'onboarding' };
}
