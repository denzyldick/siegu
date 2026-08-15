import type { MediaItem } from './media';
import type { ScanProgress, IndexingProgress } from './scan';
import type { SyncProgress, PeerDevice } from './sync';
import type { ModelProgress, DownloadProgress } from './models';

export interface ScanProgressEvent {
  payload: ScanProgress;
}

export interface IndexingProgressEvent {
  payload: IndexingProgress;
}

export interface MediaReceivedEvent {
  payload: MediaItem;
}

export interface MediaSyncedEvent {
  payload: { id: number; location: string };
}

export interface SyncProgressEvent {
  payload: SyncProgress;
}

export interface SyncErrorEvent {
  payload: { message: string; code?: string };
}

export interface ModelProgressEvent {
  payload: ModelProgress;
}

export interface DownloadProgressEvent {
  payload: DownloadProgress;
}

export interface DownloadCompleteEvent {
  payload: null;
}

export interface MediaAnalysisResultEvent {
  payload: {
    photo_id: number;
    model: string;
    status: string;
    data?: unknown;
  };
}

export interface IndexingJobEvent {
  payload: {
    status: 'running' | 'idle';
    completed?: number;
    total?: number;
  };
}

export interface IndexingEtaEvent {
  payload: {
    eta: number;
  };
}

export interface PeerConnectedEvent {
  payload: PeerDevice;
}

export interface PeerDisconnectedEvent {
  payload: string;
}

export interface WebRtcStateEvent {
  payload: string;
}

export interface RoomCodeEvent {
  payload: string;
}

export type TauriEventMap = {
  'scan-progress': ScanProgressEvent;
  'indexing-progress': IndexingProgressEvent;
  'indexing-eta': IndexingEtaEvent;
  'indexing-job': IndexingJobEvent;
  'media-received': MediaReceivedEvent;
  'media-synced': MediaSyncedEvent;
  'media-analysis-result': MediaAnalysisResultEvent;
  'sync-progress': SyncProgressEvent;
  'sync-error': SyncErrorEvent;
  'download-progress': DownloadProgressEvent;
  'download-complete': DownloadCompleteEvent;
  'model-progress': ModelProgressEvent;
  'webrtc-state': WebRtcStateEvent;
  'peer-connected': PeerConnectedEvent;
  'peer-disconnected': PeerDisconnectedEvent;
  'room-code': RoomCodeEvent;
};

export type TauriEventName = keyof TauriEventMap;
