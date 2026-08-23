export const IMAGE_EXTENSIONS = [
  'png',
  'jpg',
  'jpeg',
  'webp',
  'heic',
  'heif',
  'avif',
  'gif',
  'bmp',
  'tiff',
  'tif',
] as const;

export const VIDEO_EXTENSIONS = [
  'mp4',
  'mkv',
  'mov',
  'avi',
  'webm',
  'flv',
  'wmv',
  'm4v',
  '3gp',
] as const;

export const ALL_MEDIA_EXTENSIONS = [...IMAGE_EXTENSIONS, ...VIDEO_EXTENSIONS] as const;

export type ImageExtension = (typeof IMAGE_EXTENSIONS)[number];
export type VideoExtension = (typeof VIDEO_EXTENSIONS)[number];
export type MediaExtension = (typeof ALL_MEDIA_EXTENSIONS)[number];

export interface ExifData {
  make?: string;
  model?: string;
  lens?: string;
  focalLength?: number;
  aperture?: number;
  iso?: number;
  shutterSpeed?: string;
  dateTaken?: string;
  width?: number;
  height?: number;
  orientation?: number;
}

export interface MediaProperties {
  [key: string]: unknown;
}

export interface AiStatus {
  [model: string]: number;
}

export interface MediaItem {
  id: number;
  location: string;
  encoded: string | null;
  created: string;
  indexed: number;
  objects: Record<string, number> | null;
  properties: MediaProperties | null;
  caption: string | null;
  aesthetics_score: number | null;
  favorite: boolean;
  ai_status: AiStatus | null;
  latitude: number | null;
  longitude: number | null;
  sync_needed?: boolean;
  received?: boolean;
  view_only?: boolean;
  _groupKey?: string;
  _sortKey?: string;
}

export interface MediaGroup {
  key: string;
  label: string;
  items: MediaItem[];
}

export interface ListFilesOptions {
  offset: number;
  limit: number;
  query?: string;
  scan?: boolean;
  favoritesOnly?: boolean;
  videosOnly?: boolean;
  dateRange?: [string, string] | null;
  personIds?: string[];
  personMatch?: 'and' | 'or';
  personAlone?: boolean;
  location?: string;
  tag?: string;
  dateFrom?: string;
  dateTo?: string;
  hasFaces?: boolean;
  aestheticsMin?: number | null;
  camera?: string | null;
  papers?: boolean;
  nsfwOnly?: boolean;
  random?: boolean;
  orderBy?: string | null;
  albumId?: string | null;
}

export interface ListFilesResponse {
  photos: string;
  total: number;
  indexing_count: number;
  unindexed_count: number;
}

export function isImageFile(filename: string): boolean {
  const ext = filename.split('.').pop()?.toLowerCase() ?? '';
  return (IMAGE_EXTENSIONS as readonly string[]).includes(ext);
}

export function isVideoFile(filename: string): boolean {
  const ext = filename.split('.').pop()?.toLowerCase() ?? '';
  return (VIDEO_EXTENSIONS as readonly string[]).includes(ext);
}

export function isMediaFile(filename: string): boolean {
  return isImageFile(filename) || isVideoFile(filename);
}
