import { invoke } from '@/services/invoke';
import { getConfig } from '@/services/tauri';
import { DEFAULT_SIGNALING_URL } from '@/services/appConfig';

/** Re-export so existing callers keep a stable import. */
export { DEFAULT_SIGNALING_URL };

export interface PingResult {
  ok: boolean;
  message: string;
}

export function appendToken(url: string, token: string): string {
  const trimmed = token.trim();
  if (!trimmed) return url;
  const separator = url.includes('?') ? '&' : '?';
  return `${url}${separator}token=${encodeURIComponent(trimmed)}`;
}

/**
 * Resolve the signalling URL a user configured in Settings.
 * Falls back to the VITE_SIGNALING_URL env var, then to the managed default.
 */
export async function getConfiguredSignalingUrl(): Promise<string> {
  let url = DEFAULT_SIGNALING_URL;
  try {
    const config = await getConfig();
    if (config.signaling_url) url = config.signaling_url;
    if (config.signaling_token) url = appendToken(url, config.signaling_token);
  } catch {
    // config unavailable — keep the default
  }
  return url;
}

export async function pingSignalling(url: string): Promise<PingResult> {
  const raw = await invoke<string>('ping_signaling', { url });
  return JSON.parse(raw) as PingResult;
}

/**
 * Browser-safe signalling *base* ("host[:port]") for a Mode B guest. A guest is
 * allowed to pair by code + token against a remote `wss://` signaler, not just
 * the CLI host that served the page. Resolution order (enables the "pair from
 * anywhere" web.whatsapp.com goal, Phase 4):
 *
 *   1. `window.sieguSignalingHost` (injected by the host when it wants guests to
 *      use a hosted relay instead of its own local `/ws` bridge);
 *   2. `VITE_SIGNALING_HOST` (build-time);
 *   3. fall back to the serving origin's `/ws` bridge (`window.location.host`).
 *
 * Unlike {@link getConfiguredSignalingUrl}, this never touches the Tauri IPC, so
 * it works in a plain browser tab.
 */
export function resolveSignalingBase(): string {
  const injected = (window as { sieguSignalingHost?: string }).sieguSignalingHost;
  if (injected?.trim()) return injected.trim();
  return import.meta.env.VITE_SIGNALING_HOST || window.location.host;
}
