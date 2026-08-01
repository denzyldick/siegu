export type SyncStatus = 'idle' | 'connecting' | 'connected' | 'syncing' | 'completed' | 'error';

export type ConnectionMode = 'lan' | 'internet';

export interface SyncProgress {
  device_id: string;
  status: string;
  progress: number;
  bytes_per_second: number;
  items_completed: number;
  items_total: number;
}

export interface Device {
  id: string;
  name: string;
  photo_count: number;
  video_count: number;
  os: string;
  icon: string;
  host: string;
  last_seen: string;
}

export interface PairingCodes {
  uuid: string;
  passphrase: string[];
}

export interface SyncError {
  message: string;
  code?: string;
}

export interface DiscoveredHost {
  name: string;
  ip: string;
  port: number;
  room_id?: string;
}

export interface PeerDevice {
  device_id: string;
  name: string;
  ip: string;
  port: number;
  device_type: string;
  os: string;
  models_enabled: string[];
  protocol_version: number;
  storage_used: number;
  storage_capacity: number;
  last_seen: string;
}
