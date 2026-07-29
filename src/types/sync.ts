export type SyncStatus = 'idle' | 'connecting' | 'connected' | 'syncing' | 'completed' | 'error'

export type ConnectionMode = 'lan' | 'internet'

export interface SyncProgress {
  total: number
  received: number
  current_file: string | null
}

export interface Device {
  id: string
  name: string
  photo_count: number
  last_seen: string
}

export interface PairingCodes {
  uuid: string
  passphrase: string[]
}

export interface SyncError {
  message: string
  code?: string
}

export interface DiscoveredHost {
  name: string
  ip: string
  port: number
}

export interface PeerDevice {
  device_id: string
  name: string
  ip: string
  port: number
  device_type: string
  os: string
  models_enabled: string[]
  protocol_version: number
  storage_used: number
  storage_capacity: number
  last_seen: string
}
