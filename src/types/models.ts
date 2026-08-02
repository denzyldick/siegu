export type AiModelId =
  | 'clip'
  | 'face'
  | 'ocr'
  | 'nsfw'
  | 'aesthetics'
  | 'yolo'
  | 'blip'
  | 'midas'
  | 'whisper'

export const AI_MODEL_IDS: readonly AiModelId[] = [
  'clip', 'face', 'ocr', 'nsfw', 'aesthetics', 'yolo', 'blip', 'midas', 'whisper',
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
  face: 'Face',
  ocr: 'OCR',
  nsfw: 'NSFW',
  aesthetics: 'Aesthetics',
  yolo: 'YOLO',
  blip: 'BLIP',
  midas: 'MiDaS',
  whisper: 'Whisper',
}

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
}
