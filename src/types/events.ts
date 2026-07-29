import type { MediaItem } from './media'
import type { ScanProgress, FileScanProgress, IndexingProgress } from './scan'
import type { SyncProgress, PeerDevice } from './sync'
import type { ModelProgress, DownloadProgress, AiJob } from './models'

export interface ScanProgressEvent {
  payload: ScanProgress
}

export interface FileScanProgressEvent {
  payload: FileScanProgress
}

export interface IndexingProgressEvent {
  payload: IndexingProgress
}

export interface MediaReceivedEvent {
  payload: MediaItem
}

export interface MediaSyncedEvent {
  payload: { id: number; location: string }
}

export interface SyncProgressEvent {
  payload: SyncProgress
}

export interface SyncErrorEvent {
  payload: { message: string; code?: string }
}

export interface ModelProgressEvent {
  payload: ModelProgress
}

export interface DownloadProgressEvent {
  payload: DownloadProgress
}

export interface MediaAnalysisResultEvent {
  payload: {
    photo_id: number
    model: string
    status: string
    data?: unknown
  }
}

export interface AiJobEvent {
  payload: AiJob
}

export interface IndexingEtaEvent {
  payload: {
    model: string
    eta: number
  }
}

export interface PeerConnectedEvent {
  payload: PeerDevice
}

export interface PeerDisconnectedEvent {
  payload: string
}

export interface WebRtcStateEvent {
  payload: string
}

export interface RoomCodeEvent {
  payload: string
}

export type TauriEventMap = {
  'scan-progress': ScanProgressEvent
  'file-scan-progress': FileScanProgressEvent
  'indexing-progress': IndexingProgressEvent
  'indexing-eta': IndexingEtaEvent
  'media-received': MediaReceivedEvent
  'media-synced': MediaSyncedEvent
  'media-analysis-result': MediaAnalysisResultEvent
  'current-ai-job': AiJobEvent
  'sync-progress': SyncProgressEvent
  'sync-error': SyncErrorEvent
  'download-progress': DownloadProgressEvent
  'model-progress': ModelProgressEvent
  'webrtc-state': WebRtcStateEvent
  'peer-connected': PeerConnectedEvent
  'peer-disconnected': PeerDisconnectedEvent
  'room-code': RoomCodeEvent
}

export type TauriEventName = keyof TauriEventMap
