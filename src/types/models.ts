export type AiModelId =
  'clip' | 'face' | 'ocr' | 'nsfw' | 'aesthetics' | 'yolo' | 'blip' | 'midas' | 'whisper';

export const AI_MODEL_IDS: readonly AiModelId[] = [
  'clip',
  'face',
  'ocr',
  'nsfw',
  'aesthetics',
  'yolo',
  'blip',
  'midas',
  'whisper',
] as const;

export interface AiModelInfo {
  id: AiModelId;
  name: string;
  description: string;
  size: string;
  downloaded: boolean;
}

export interface ModelProgress {
  model: string;
  pending: number | null;
  total: number | null;
  status: string;
  message: string;
}

export interface DownloadProgress {
  model: string;
  downloaded: number;
  total: number | null;
}

export interface ModelCapability {
  model: string;
  runnable: boolean;
  reason: string | null;
}

export type ModelBlockReason = 'low_ram' | 'memory_budget' | 'load_failed';

/** Reason codes that mean a model cannot run on this device at all. */
export const MODEL_BLOCK_REASONS: readonly ModelBlockReason[] = [
  'low_ram',
  'memory_budget',
  'load_failed',
];

export interface AiJob {
  photo_id: number;
  model: AiModelId;
  status: 'pending' | 'running' | 'completed' | 'failed';
}

export const MODEL_DISPLAY_NAMES: Record<AiModelId, string> = {
  clip: 'CLIP',
  face: 'Face',
  ocr: 'OCR',
  nsfw: 'NSFW',
  aesthetics: 'Aesthetics',
  yolo: 'YOLO',
  blip: 'BLIP',
  midas: 'MiDaS',
  whisper: 'Whisper',
};

export const MODEL_DESCRIPTIONS: Record<AiModelId, string> = {
  clip: 'Image-text understanding and search',
  face: 'Face detection, recognition and grouping',
  ocr: 'Text extraction from images',
  nsfw: 'Content safety filtering',
  aesthetics: 'Photo quality scoring',
  yolo: 'Object detection',
  blip: 'Image captioning',
  midas: 'Depth estimation',
  whisper: 'Audio transcription for videos',
};
