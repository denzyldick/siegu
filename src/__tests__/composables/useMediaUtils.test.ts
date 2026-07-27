import { describe, it, expect } from 'vitest'
import {
  isVideo,
  formatEta,
  formatBytes,
  formatScore,
  normalizeIndexingCount,
  getExtension,
  VIDEO_EXTENSIONS_LIST,
} from '@/composables/useMediaUtils'

describe('useMediaUtils', () => {
  describe('isVideo', () => {
    it('returns true for all video formats', () => {
      expect(isVideo('clip.mp4')).toBe(true)
      expect(isVideo('clip.mkv')).toBe(true)
      expect(isVideo('clip.MOV')).toBe(true)
      expect(isVideo('clip.avi')).toBe(true)
      expect(isVideo('clip.webm')).toBe(true)
      expect(isVideo('clip.flv')).toBe(true)
      expect(isVideo('clip.wmv')).toBe(true)
      expect(isVideo('clip.m4v')).toBe(true)
      expect(isVideo('clip.3gp')).toBe(true)
    })

    it('returns false for image formats', () => {
      expect(isVideo('photo.jpg')).toBe(false)
      expect(isVideo('photo.png')).toBe(false)
      expect(isVideo('photo.heic')).toBe(false)
      expect(isVideo('photo.webp')).toBe(false)
    })

    it('returns false for empty or invalid input', () => {
      expect(isVideo('')).toBe(false)
      expect(isVideo('noextension')).toBe(false)
    })

    it('is case insensitive', () => {
      expect(isVideo('VIDEO.MP4')).toBe(true)
      expect(isVideo('video.WebM')).toBe(true)
    })
  })

  describe('formatEta', () => {
    it('formats seconds under 60', () => {
      expect(formatEta(0)).toBe('0s')
      expect(formatEta(30)).toBe('30s')
      expect(formatEta(59)).toBe('59s')
    })

    it('formats minutes and seconds', () => {
      expect(formatEta(60)).toBe('1m 0s')
      expect(formatEta(90)).toBe('1m 30s')
      expect(formatEta(119)).toBe('1m 59s')
    })

    it('formats hours and minutes', () => {
      expect(formatEta(3600)).toBe('1h 0m')
      expect(formatEta(3660)).toBe('1h 1m')
      expect(formatEta(7200)).toBe('2h 0m')
    })

    it('returns empty string for invalid input', () => {
      expect(formatEta(NaN)).toBe('')
      expect(formatEta(-1)).toBe('')
      expect(formatEta(Infinity)).toBe('')
    })
  })

  describe('formatBytes', () => {
    it('formats bytes', () => {
      expect(formatBytes(0)).toBe('0 B')
      expect(formatBytes(100)).toBe('100 B')
    })

    it('formats kilobytes', () => {
      expect(formatBytes(1024)).toBe('1.0 KB')
      expect(formatBytes(1536)).toBe('1.5 KB')
    })

    it('formats megabytes', () => {
      expect(formatBytes(1048576)).toBe('1.0 MB')
      expect(formatBytes(5242880)).toBe('5.0 MB')
    })

    it('formats gigabytes', () => {
      expect(formatBytes(1073741824)).toBe('1.0 GB')
    })
  })

  describe('formatScore', () => {
    it('formats valid scores', () => {
      expect(formatScore(0.85)).toBe('85%')
      expect(formatScore(1)).toBe('100%')
      expect(formatScore(0)).toBe('0%')
    })

    it('returns empty for null/undefined', () => {
      expect(formatScore(null)).toBe('')
      expect(formatScore(undefined as unknown as null)).toBe('')
    })
  })

  describe('normalizeIndexingCount', () => {
    it('returns valid numbers', () => {
      expect(normalizeIndexingCount(0)).toBe(0)
      expect(normalizeIndexingCount(42)).toBe(42)
      expect(normalizeIndexingCount('100')).toBe(100)
    })

    it('returns 0 for invalid input', () => {
      expect(normalizeIndexingCount(NaN)).toBe(0)
      expect(normalizeIndexingCount('abc')).toBe(0)
      expect(normalizeIndexingCount(null)).toBe(0)
      expect(normalizeIndexingCount(undefined)).toBe(0)
    })
  })

  describe('getExtension', () => {
    it('extracts file extension', () => {
      expect(getExtension('photo.jpg')).toBe('jpg')
      expect(getExtension('video.MP4')).toBe('mp4')
      expect(getExtension('path/to/file.heic')).toBe('heic')
    })

    it('returns empty string for no extension', () => {
      expect(getExtension('noextension')).toBe('')
      expect(getExtension('')).toBe('')
    })
  })

  describe('VIDEO_EXTENSIONS_LIST', () => {
    it('contains all 9 video formats', () => {
      expect(VIDEO_EXTENSIONS_LIST).toHaveLength(9)
      expect(VIDEO_EXTENSIONS_LIST).toContain('mp4')
      expect(VIDEO_EXTENSIONS_LIST).toContain('mkv')
      expect(VIDEO_EXTENSIONS_LIST).toContain('mov')
      expect(VIDEO_EXTENSIONS_LIST).toContain('avi')
      expect(VIDEO_EXTENSIONS_LIST).toContain('webm')
      expect(VIDEO_EXTENSIONS_LIST).toContain('flv')
      expect(VIDEO_EXTENSIONS_LIST).toContain('wmv')
      expect(VIDEO_EXTENSIONS_LIST).toContain('m4v')
      expect(VIDEO_EXTENSIONS_LIST).toContain('3gp')
    })
  })
})
