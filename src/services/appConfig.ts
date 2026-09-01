/**
 * Single source of truth for Siegu's base domain (#share).
 *
 * Everything that points at the Siegu web property derives from `APP_BASE_HOST`,
 * so switching to a custom base (or back to the managed `siegu.io`) is a
 * one-line change. For now, distribution is local: the desktop app generates
 * share links that point at `127.0.0.1` (see `startAlbumShare`). When the
 * hosted relay + web origin go live, the share origin flips to `APP_WEB_BASE`
 * and the signalling URL stops being loopback — with no other changes needed.
 *
 * Overrides (build/dev):
 *   - `VITE_SIGNALING_URL` replaces the default signalling WebSocket URL.
 *   - `VITE_SHARE_ORIGIN` replaces where generated share links point.
 */
export const APP_BASE_HOST = import.meta.env.VITE_APP_BASE_HOST || 'siegu.io';

export const APP_WEB_BASE = `https://${APP_BASE_HOST}`;

/** Public marketing/help entry point for sharing (`siegu.io/connect`). */
export const APP_CONNECT_URL = `${APP_WEB_BASE}/connect`;

/** Landing page. Overridable via `VITE_APP_LANDING_URL` to run it locally. */
export const APP_LANDING_URL = import.meta.env.VITE_APP_LANDING_URL || APP_WEB_BASE;

/** Source repository (open source project). */
export const APP_GITHUB_URL =
  import.meta.env.VITE_APP_GITHUB_URL || 'https://github.com/denzyldick/siegu';

/** User documentation (source docs folder on GitHub). */
export const APP_DOCS_URL = import.meta.env.VITE_APP_DOCS_URL || `${APP_GITHUB_URL}/tree/main/docs`;

/** Signalling WebSocket endpoint default (`wss://siegu.io/ws`). */
export const DEFAULT_SIGNALING_URL =
  import.meta.env.VITE_SIGNALING_URL || `wss://${APP_BASE_HOST}/ws`;

/** Where guests open generated share links (`https://siegu.io` once hosted). */
export const DEFAULT_SHARE_ORIGIN = import.meta.env.VITE_SHARE_ORIGIN || '';
