import type { DownloadProgressState } from '@/types/settings';

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  const decimals = value >= 10 || unitIndex === 0 ? 0 : value >= 1 ? 1 : 2;
  return `${value.toFixed(decimals)} ${units[unitIndex]}`;
}

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return '';
  const totalSeconds = Math.round(ms / 1000);
  if (totalSeconds < 60) return `${totalSeconds}s`;
  const minutes = Math.floor(totalSeconds / 60);
  if (minutes < 60) return `${minutes}m ${totalSeconds % 60}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

export function updateDownloadProgress(
  prev: DownloadProgressState | undefined,
  downloaded: number,
  total: number | null,
  now: number,
): DownloadProgressState {
  let speedBytesPerSec = 0;
  let etaMs: number | null = null;
  if (prev && prev.updatedAt > 0) {
    const dt = now - prev.updatedAt;
    if (dt > 0 && downloaded >= prev.downloaded) {
      const instant = ((downloaded - prev.downloaded) * 1000) / dt;
      speedBytesPerSec =
        prev.speedBytesPerSec > 0 ? prev.speedBytesPerSec * 0.5 + instant * 0.5 : instant;
    }
  }
  if (speedBytesPerSec > 0 && total != null && total > 0 && downloaded < total) {
    etaMs = ((total - downloaded) / speedBytesPerSec) * 1000;
  }
  return { downloaded, total, speedBytesPerSec, etaMs, updatedAt: now };
}
