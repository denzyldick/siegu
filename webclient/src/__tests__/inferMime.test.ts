import { describe, it, expect } from 'vitest';
import { inferMime } from '../lib';

describe('inferMime', () => {
  it('returns video/mp4 for .mp4', () => {
    expect(inferMime('photo.mp4')).toBe('video/mp4');
  });

  it('returns video/mp4 for .mov', () => {
    expect(inferMime('clip.mov')).toBe('video/mp4');
  });

  it('returns video/mp4 for .m4v', () => {
    expect(inferMime('video.m4v')).toBe('video/mp4');
  });

  it('returns video/webm for .webm', () => {
    expect(inferMime('stream.webm')).toBe('video/webm');
  });

  it('returns image/jpeg for .jpg', () => {
    expect(inferMime('photo.jpg')).toBe('image/jpeg');
  });

  it('returns image/jpeg for .jpeg', () => {
    expect(inferMime('photo.jpeg')).toBe('image/jpeg');
  });

  it('returns image/jpeg for .png (fallback)', () => {
    expect(inferMime('photo.png')).toBe('image/jpeg');
  });

  it('returns image/jpeg for .heic (fallback)', () => {
    expect(inferMime('photo.heic')).toBe('image/jpeg');
  });

  it('is case-insensitive', () => {
    expect(inferMime('VIDEO.MP4')).toBe('video/mp4');
    expect(inferMime('Photo.WEBM')).toBe('video/webm');
  });
});
