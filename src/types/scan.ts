export type ScanStatus = 'idle' | 'scanning' | 'indexing' | 'completed' | 'error'

export interface ScanProgress {
  files_found: number
  files_processed: number
  current_file: string | null
}

export interface FileScanProgress {
  file: string
  status: 'pending' | 'scanning' | 'completed'
}

export interface IndexingProgress {
  total: number
  completed: number
  model: string
  eta: number
}

export interface ScanFile {
  id: number
  location: string
  encoded: string | null
  created: string
}

export interface Directory {
  path: string
  id: number
}
