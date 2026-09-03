export type UpdateStatus = 'idle' | 'checking' | 'available' | 'uptodate' | 'downloading' | 'error';

export type ModelProcessingStatus =
  'idle' | 'starting' | 'running' | 'completed' | 'up_to_date' | 'unavailable' | 'error';

export type IndexingMode = 'immediate' | 'idle' | 'manual';

export interface DirectoryEntry {
  title: string;
  value: string;
}

export type LogLevel = 'fatal' | 'error' | 'warn' | 'info' | 'debug' | 'trace';

export interface LogEntry {
  time: string;
  message: string;
  type: 'error' | 'warn' | 'info' | 'debug';
  level: LogLevel;
}

export interface DownloadProgressState {
  downloaded: number;
  total: number | null;
  speedBytesPerSec: number;
  etaMs: number | null;
  updatedAt: number;
}

export interface DownloadStats {
  bytesText: string;
  rightText: string;
  hasTotal: boolean;
}

export interface ModelProgressState {
  pending: number | null;
  total: number | null;
  status: ModelProcessingStatus;
  message: string;
  updatedAt: number;
}

export type PerformancePreset = 'low' | 'balanced' | 'full' | 'custom';

export interface PerformanceConfig {
  scanThreads: number;
  mlThreads: number;
  batchDelayMs: number;
  memoryBudgetMb: number;
  indexingMode: IndexingMode;
}

export interface CleanupDialogState {
  show: boolean;
}

export interface RemoveFolderDialogState {
  show: boolean;
  path: string;
}

export interface SnackbarState {
  show: boolean;
  text: string;
  error: boolean;
}

export interface ProStatus {
  ok: boolean;
  paid: boolean;
  verified: boolean;
  sent?: boolean;
  plan?: string;
  email?: string;
  error?: string;
  status?: number;
}
