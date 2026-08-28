/**
 * Backend abstraction (#19, Phase 1).
 *
 * `GuestClient` + `createPeerTransport` let the browser Vue app act as a
 * read-only (or rw) guest against a hosting `siegu web` device. A future PR
 * wires this in as the "guest" impl of a common `Backend` interface alongside
 * the existing Tauri client.
 */
export { GuestClient } from './guest';
export type { GuestEvents } from './guest';
export { createPeerTransport } from './peer';
export type { PeerTransport } from './peer';
export { parseHash, inferMime, assembleChunks, FileAssembler } from './protocol';
export type { GuestSession, GuestInbound, GuestOutbound } from './protocol';
