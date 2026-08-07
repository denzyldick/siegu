import { describe, it, expect, beforeEach } from 'vitest';
import { isVideo } from '@/composables/useMediaUtils';
import { createMediaItem, createVideoItem } from '../helpers/factories';

describe('critical path: video renders in grid', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('detects all 9 video formats', () => {
    const formats = ['mp4', 'mkv', 'mov', 'avi', 'webm', 'flv', 'wmv', 'm4v', '3gp'];
    for (const ext of formats) {
      const item = createVideoItem({ location: `/videos/test.${ext}` });
      expect(isVideo(item.location)).toBe(true);
    }
  });

  it('does not detect images as video', () => {
    const item = createMediaItem({ location: '/photos/test.jpg' });
    expect(isVideo(item.location)).toBe(false);
  });

  it('creates valid video item', () => {
    const item = createVideoItem();
    expect(item.location).toContain('.mp4');
    expect(item.id).toBeGreaterThan(0);
  });

  it('creates valid photo item', () => {
    const item = createMediaItem();
    expect(item.location).toContain('.jpg');
    expect(item.id).toBeGreaterThan(0);
  });
});
