import { describe, it, expect } from 'vitest';
import {
  isImageFile,
  isVideoFile,
  isMediaFile,
  IMAGE_EXTENSIONS,
  VIDEO_EXTENSIONS,
} from '@/types/media';

describe('media type utilities', () => {
  describe('isImageFile', () => {
    it('recognizes all image formats', () => {
      for (const ext of IMAGE_EXTENSIONS) {
        expect(isImageFile(`photo.${ext}`)).toBe(true);
        expect(isImageFile(`photo.${ext.toUpperCase()}`)).toBe(true);
      }
    });

    it('rejects video formats', () => {
      expect(isImageFile('video.mp4')).toBe(false);
      expect(isImageFile('video.mkv')).toBe(false);
    });
  });

  describe('isVideoFile', () => {
    it('recognizes all 9 video formats', () => {
      for (const ext of VIDEO_EXTENSIONS) {
        expect(isVideoFile(`video.${ext}`)).toBe(true);
        expect(isVideoFile(`video.${ext.toUpperCase()}`)).toBe(true);
      }
    });

    it('rejects image formats', () => {
      expect(isVideoFile('photo.jpg')).toBe(false);
      expect(isVideoFile('photo.png')).toBe(false);
    });
  });

  describe('isMediaFile', () => {
    it('recognizes both image and video formats', () => {
      expect(isMediaFile('photo.jpg')).toBe(true);
      expect(isMediaFile('video.mp4')).toBe(true);
      expect(isMediaFile('video.flv')).toBe(true);
      expect(isMediaFile('photo.heic')).toBe(true);
    });

    it('rejects non-media files', () => {
      expect(isMediaFile('document.pdf')).toBe(false);
      expect(isMediaFile('script.js')).toBe(false);
    });
  });
});
