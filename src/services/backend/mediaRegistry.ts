/**
 * Module-level registry for the active media backend resolver (#24, Phase 1).
 *
 * `useMediaUrl`'s mode-aware resolvers need to reach the active `Backend`
 * (webHost/guest/tauri) WITHOUT importing a store: stores make the composable
 * depend on an active Pinia instance, which breaks unit tests and creates a
 * circular init path (`tauriBackend` → `useMediaUrl` → store → `tauriBackend`).
 *
 * Instead, `useRuntimeStore.initRuntime()` registers a resolver here once it
 * has detected the mode; `useMediaUrl` consults it lazily and falls back to the
 * local Tauri media-server URLs when nothing is registered (desktop, not yet
 * booted). This keeps the composable pure and usable outside components.
 */
import type { Backend, MediaKind } from './interface';
import type { MediaItem } from '@/types/media';

type BackendResolver = () => Backend | null;

let getActiveBackend: BackendResolver | null = null;

/** Register (or clear with `null`) the resolver for the runtime's active backend. */
export function registerMediaBackend(resolver: BackendResolver | null): void {
  getActiveBackend = resolver;
}

/** The active backend, if the runtime has initialized and registered one. */
export function activeMediaBackend(): Backend | null {
  return getActiveBackend ? getActiveBackend() : null;
}

/** Resolve a media src for `item` through the active backend seam, or null. */
export async function resolveBackendMedia(
  item: MediaItem,
  kind: MediaKind,
): Promise<string | null> {
  const backend = activeMediaBackend();
  if (!backend) return null;
  try {
    return await backend.mediaUrl(item.id, kind);
  } catch {
    return null;
  }
}