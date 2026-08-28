/**
 * Backend abstraction (#19, Phase 1).
 *
 * `GuestClient` + `createPeerTransport` let the browser Vue app act as a
 * guest against a hosting `siegu web` device. The `Backend` interface is the
 * common seam between the local Tauri client (`tauriBackend`) and the guest
 * remote client (`guestBackend`); `createBackend` picks one by mode.
 */
export { createBackend } from './createBackend';
export { GuestClient } from './guest';
export type { GuestEvents } from './guest';
export { createPeerTransport } from './peer';
export type { PeerTransport } from './peer';
export { parseHash, inferMime, assembleChunks, FileAssembler } from './protocol';
export type { GuestSession, GuestInbound, GuestOutbound } from './protocol';
export { tauriBackend } from './tauriBackend';
export { guestBackend } from './guestBackend';
export type { Backend, BackendMode, MediaKind } from './interface';
export { mediaCacheKey } from './interface';
