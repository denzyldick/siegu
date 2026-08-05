import { describe, it, expect } from 'vitest';
import { formatBytes, formatDuration, updateDownloadProgress } from '@/utils/format';

describe('formatBytes', () => {
  it('formats plain bytes', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(512)).toBe('512 B');
  });

  it('formats kB/MB/GB with sensible precision', () => {
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(10 * 1024)).toBe('10 KB');
    expect(formatBytes(612 * 1024 * 1024)).toBe('612 MB');
    expect(formatBytes(1.5 * 1024 * 1024 * 1024)).toBe('1.5 GB');
  });

  it('handles invalid input', () => {
    expect(formatBytes(Number.NaN)).toBe('0 B');
    expect(formatBytes(-1)).toBe('0 B');
  });
});

describe('formatDuration', () => {
  it('renders seconds, minutes and hours', () => {
    expect(formatDuration(4500)).toBe('5s');
    expect(formatDuration(90_000)).toBe('1m 30s');
    expect(formatDuration(7_200_000)).toBe('2h 0m');
  });

  it('returns empty for invalid input', () => {
    expect(formatDuration(0)).toBe('');
    expect(formatDuration(Number.NaN)).toBe('');
  });
});

describe('updateDownloadProgress', () => {
  it('returns a zeroed baseline on the first event (no speed spike)', () => {
    const state = updateDownloadProgress(undefined, 290_000_000, 647_000_000, 10_000);
    expect(state).toEqual({
      downloaded: 290_000_000,
      total: 647_000_000,
      speedBytesPerSec: 0,
      etaMs: null,
      updatedAt: 10_000,
    });
  });

  it('computes speed and ETA from consecutive events', () => {
    const first = updateDownloadProgress(undefined, 1_000_000, 10_000_000, 1_000);
    const second = updateDownloadProgress(first, 2_000_000, 10_000_000, 2_000);
    expect(second.speedBytesPerSec).toBe(1_000_000); // 1MB in 1s
    expect(second.etaMs).toBe(8_000); // 8MB remaining at 1MB/s
  });

  it('smooths speed with the previous estimate', () => {
    const first = updateDownloadProgress(undefined, 1_000_000, 10_000_000, 1_000);
    const second = updateDownloadProgress(first, 2_000_000, 10_000_000, 2_000); // 1MB/s
    const third = updateDownloadProgress(second, 4_000_000, 10_000_000, 3_000); // 2MB/s instant
    expect(third.speedBytesPerSec).toBe(1_500_000); // 0.5*1MB/s + 0.5*2MB/s
    expect(third.etaMs).toBe(4_000); // 6MB remaining at 1.5MB/s
  });

  it('drops ETA once the total is reached', () => {
    const first = updateDownloadProgress(undefined, 1_000_000, 2_000_000, 1_000);
    const done = updateDownloadProgress(first, 2_000_000, 2_000_000, 2_000);
    expect(done.etaMs).toBeNull();
  });

  it('handles a resumed download without an artificial spike', () => {
    // First event after resume already carries the partial bytes; baseline is set
    // on the first event (updatedAt 0) so speed stays 0 until a second sample.
    const resumed = updateDownloadProgress(
      { downloaded: 0, total: 1, speedBytesPerSec: 0, etaMs: null, updatedAt: 0 },
      277_000_000,
      647_000_000,
      5_000,
    );
    expect(resumed.speedBytesPerSec).toBe(0);
    expect(resumed.etaMs).toBeNull();
  });
});
