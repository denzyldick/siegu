export type UpdateStatus = 'idle' | 'checking' | 'available' | 'uptodate' | 'downloading' | 'error';

export type ModelProcessingStatus =
  'idle' | 'starting' | 'running' | 'completed' | 'up_to_date' | 'unavailable' | 'error';

export type IndexingMode = 'immediate' | 'idle' | 'manual';

export interface DirectoryEntry {
  title: string;
  value: string;
}

export interface LogEntry {
  time: string;
  message: string;
  type: 'error' | 'info';
}

export interface DownloadProgressState {
  downloaded: number;
  total: number;
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

export interface DownloadDialogState {
  show: boolean;
  title: string;
  message: string;
  models: string[];
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
