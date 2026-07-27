export type AiModelId =
  | 'clip'
  | 'ultraface'
  | 'ocr'
  | 'nsfw'
  | 'aesthetics'
  | 'yolo'
  | 'blip'
  | 'arcface'
  | 'midas'
  | 'whisper'

export const AI_MODEL_IDS: readonly AiModelId[] = [
  'clip', 'ultraface', 'ocr', 'nsfw', 'aesthetics', 'yolo', 'blip', 'arcface', 'midas', 'whisper',
] as const

export interface AiModelInfo {
  id: AiModelId
  name: string
  description: string
  size: string
  downloaded: boolean
}

export interface ModelProgress {
  model: string
  progress: number
  eta: number
}

export interface DownloadProgress {
  model: string
  progress: number
  speed: number
}

export interface AiJob {
  photo_id: number
  model: AiModelId
  status: 'pending' | 'running' | 'completed' | 'failed'
}

export const MODEL_DISPLAY_NAMES: Record<AiModelId, string> = {
  clip: 'CLIP',
  ultraface: 'UltraFace',
  ocr: 'OCR',
  nsfw: 'NSFW',
  aesthetics: 'Aesthetics',
  yolo: 'YOLO',
  blip: 'BLIP',
  arcface: 'ArcFace',
  midas: 'MiDaS',
  whisper: 'Whisper',
}

export const MODEL_DESCRIPTIONS: Record<AiModelId, string> = {
  clip: 'Image-text understanding and search',
  ultraface: 'Face detection',
  ocr: 'Text extraction from images',
  nsfw: 'Content safety filtering',
  aesthetics: 'Photo quality scoring',
  yolo: 'Object detection',
  blip: 'Image captioning',
  arcface: 'Face recognition and grouping',
  midas: 'Depth estimation',
  whisper: 'Audio transcription for videos',
}
