import { tauriBackend } from './tauriBackend';
import { guestBackend } from './guestBackend';
import { webHostBackend } from './webHostBackend';
import type { GuestClient } from './guest';
import type { Backend, BackendMode } from './interface';

/**
 * Pick a `Backend` by mode (#24):
 *  - `tauri`   — local desktop client (default)
 *  - `webHost` — browser owner of a `siegu web` library, over HTTP/RPC (Mode A)
 *  - `guest`   — browser pairing to a remote Siegu over WebRTC (Mode B); needs a
 *                connected `GuestClient`
 */
export function createBackend(
  mode: BackendMode,
  client?: GuestClient,
  webHostToken?: string,
): Backend {
  switch (mode) {
    case 'webHost':
      return webHostBackend(webHostToken);
    case 'guest':
      return client ? guestBackend(client) : tauriBackend();
    case 'tauri':
    default:
      return tauriBackend();
  }
}
