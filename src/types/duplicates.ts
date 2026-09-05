// Duplicate-detection types (see crates/siegu-core/src/duplicates.rs).

export interface DuplicateMemberView {
  id: string;
  location: string;
  aesthetics: number | null;
  is_best: boolean;
}

export interface DuplicateGroupView {
  members: DuplicateMemberView[];
  best_id: string | null;
  unknown_best: boolean;
  kind: 'exact' | 'perceptual' | 'clip';
  reclaimable_bytes: number;
}

export interface DuplicateStats {
  group_count: number;
  duplicate_count: number;
  reclaimable_bytes: number;
}

export interface LibraryOverview {
  photo_count: number;
  video_count: number;
  library_bytes: number;
}

export interface DuplicateScanResult {
  groups: DuplicateGroupView[];
  stats: DuplicateStats;
  library_bytes: number;
  photo_count: number;
  video_count: number;
}

export interface ScanProgress {
  done: number;
  total: number;
}