export type ScanStatus = 'idle' | 'scanning' | 'indexing' | 'completed' | 'error';
export type ScanPhase = 'idle' | 'discovering' | 'processing' | 'indexing' | 'paused' | 'complete';

export interface ScanProgress {
  status: 'discovering' | 'indexing' | 'complete' | 'paused';
  progress?: number;
  current?: number;
  total?: number;
  current_directory?: string;
  message?: string;
  files_found?: number;
  files_processed?: number;
  current_file?: string | null;
}

export interface IndexingProgress {
  remaining: number;
}

export interface AnalysisActivity {
  id: string;
  location: string;
  models: string[];
  remaining: number;
}

export interface ModelState {
  pending: number;
  total: number;
  status: string;
  message?: string;
}

export interface ScanFile {
  id: number;
  location: string;
  encoded: string | null;
  created: string;
}

export interface Directory {
  path: string;
  id: number;
}
