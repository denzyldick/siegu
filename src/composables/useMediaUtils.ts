import { isVideoFile, VIDEO_EXTENSIONS, IMAGE_EXTENSIONS } from '@/types/media'
import { convertFileSrc } from '@tauri-apps/api/core'

export const VIDEO_EXTENSIONS_LIST = [...VIDEO_EXTENSIONS] as readonly string[]
export const IMAGE_EXTENSIONS_LIST = [...IMAGE_EXTENSIONS] as readonly string[]

export function isVideo(filename: string): boolean {
  return isVideoFile(filename)
}

export function getExtension(filename: string): string {
  const dotIndex = filename.lastIndexOf('.')
  if (dotIndex === -1 || dotIndex === filename.length - 1) return ''
  return filename.slice(dotIndex + 1).toLowerCase()
}

export function formatEta(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds < 0) return ''
  if (totalSeconds < 60) return `${Math.round(totalSeconds)}s`

  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = Math.round(totalSeconds % 60)

  if (hours > 0) {
    return `${hours}h ${minutes}m`
  }
  return `${minutes}m ${seconds}s`
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const k = 1024
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  const value = bytes / Math.pow(k, i)
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

export function formatScore(score: number | null): string {
  if (score === null || score === undefined) return ''
  return `${Math.round(score * 100)}%`
}

export function normalizeIndexingCount(value: unknown): number {
  const n = Number(value)
  return Number.isFinite(n) ? n : 0
}

export function getFaceImageSrc(cropPath: string | null, encoded: string | null): string {
  if (encoded) return encoded
  if (cropPath) return convertFileSrc(cropPath)
  return ''
}

export function getMediaThumbnailSrc(
  location: string,
  encoded: string | null,
  useFileSrc: boolean = false,
): string {
  if (encoded) return encoded
  if (useFileSrc) return convertFileSrc(location)
  return ''
}
