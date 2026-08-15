export type ScanStatus = 'idle' | 'scanning' | 'indexing' | 'completed' | 'error';

export interface ScanProgress {
  status: 'discovering' | 'indexing' | 'complete';
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
