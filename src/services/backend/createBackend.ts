import { tauriBackend } from './tauriBackend';
import { guestBackend } from './guestBackend';
import type { GuestClient } from './guest';
import type { Backend, BackendMode } from './interface';

/**
 * Pick a `Backend` by mode. `guest` requires a connected `GuestClient`;
 * `tauri` is the default local client.
 */
export function createBackend(mode: BackendMode, client?: GuestClient): Backend {
  return mode === 'guest' && client ? guestBackend(client) : tauriBackend();
}
