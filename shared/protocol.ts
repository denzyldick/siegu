/**
 * Shared protocol types for Siegu's sync and signalling layer.
 *
 * ⚠️  Keep in sync with:
 *   - `crates/siegu-core/src/mesh.rs`   (SyncMessage, SyncPhase)
 *   - `crates/siegu-core/src/signal.rs` (SignalMessage)
 *   - `crates/siegu-core/src/database.rs` (PhotoSyncInfo, SyncObject, SyncFace)
 *
 * These types are used by:
 *   - The desktop app (Rust, via serde)
 *   - The web client (TypeScript, parsed from JSON data-channel messages)
 */

// ── Data-channel protocol (WebRTC) ──────────────────────────────────────────

export type SyncPhase = 'idle' | 'syncing' | 'completed';

export interface PhotoSyncInfo {
  id: string;
  location: string;
  created: string;
  latitude: number | null;
  longitude: number | null;
  objects: string;   // JSON array of SyncObject
  faces: string;     // JSON array of SyncFace
  caption: string | null;
  aesthetics_score: number | null;
}

export interface SyncObject {
  class: string;
  probability: string;
}

export interface SyncFace {
  face_id: string;
  crop_path: string;
  encoded: string;
  person_id: string | null;
}

export type SyncMessage =
  | { type: 'ManifestRequest' }
  | { type: 'ManifestResponse'; photos: PhotoSyncInfo[]; more?: boolean }
  | { type: 'FileRequest'; id: string }
  | {
      type: 'FileHeader';
      id: string;
      filename: string;
      relative_path: string;
      size: number;
      checksum: string;
      created: string;
      latitude: number | null;
      longitude: number | null;
      objects: string;
      faces: string;
      caption: string | null;
      aesthetics_score: number | null;
      encoded?: string;
    }
  | { type: 'FileChunk'; id: string; index: number; data: number[] }
  | { type: 'FileEnd'; id: string; checksum: string }
  | { type: 'SyncFile'; photo: PhotoSyncInfo }
  | { type: 'StartSync' }
  | { type: 'CatchUp' }
  | {
      type: 'PeerProgress';
      status: string;
      phase?: SyncPhase;
      progress: number;
      items_completed: number;
      items_total: number;
    }
  | {
      type: 'MetadataUpdate';
      photo_id: string;
      caption: string | null;
      aesthetics_score: number | null;
      indexed: number;
      deleted_at?: string | null;
    }
  | {
      type: 'VersionNegotiate';
      version: number;
      device_id: string;
      device_name: string;
      os: string;
      models_enabled: string[];
    }
  | { type: 'VersionReject'; reason: string };

// ── Signalling protocol (WebSocket) ─────────────────────────────────────────

export type SignalMessage =
  | { type: 'join'; device_id: string; token?: string }
  | { type: 'joined'; device_id: string; room_id: string; peer_count: number }
  | { type: 'offer'; payload: string; target: string }
  | { type: 'answer'; payload: string; target: string }
  | { type: 'ice_candidate'; payload: string; target: string }
  | { type: 'peer_disconnected'; device_id: string }
  | { type: 'peer_joined'; device_id: string }
  | { type: 'error'; message: string }
  | { type: 'create_room'; token?: string }
  | { type: 'room_created'; code: string }
  | { type: 'join_room'; code: string; token?: string }
  | { type: 'room_joined' }
  | { type: 'relay'; from?: string; payload: unknown }
  | { type: 'peer_list'; peers: string[] }
  | { type: 'device_announce'; device_id: string; metadata: unknown }
  | { type: 'room_closed' };

// ── Protocol constants ──────────────────────────────────────────────────────

export const PROTOCOL_VERSION = 2;
export const MAX_MESH_DEVICES = 5;
export const FILE_CHUNK_PAYLOAD = 14_000;
export const SYNC_MESSAGE_BUDGET = 48_000;
export const MAX_DATA_CHANNEL_MSG_SIZE = 60_000;
