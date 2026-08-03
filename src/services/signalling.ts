import { invoke } from '@tauri-apps/api/core';
import { getConfig } from '@/services/tauri';

export const DEFAULT_SIGNALING_URL = import.meta.env.VITE_SIGNALING_URL || 'wss://siegu.io/ws';

export interface PingResult {
  ok: boolean;
  message: string;
}

export function appendToken(url: string, token: string): string {
  const trimmed = token.trim();
  if (!trimmed) return url;
  const separator = url.includes('?') ? '&' : '?';
  return `${url}${separator}token=${encodeURIComponent(trimmed)}`;
}

/**
 * Resolve the signalling URL a user configured in Settings.
 * Falls back to the VITE_SIGNALING_URL env var, then to the managed default.
 */
export async function getConfiguredSignalingUrl(): Promise<string> {
  let url = DEFAULT_SIGNALING_URL;
  try {
    const config = await getConfig();
    if (config.signaling_url) url = config.signaling_url;
    if (config.signaling_token) url = appendToken(url, config.signaling_token);
  } catch {
    // config unavailable — keep the default
  }
  return url;
}

export async function pingSignalling(url: string): Promise<PingResult> {
  const raw = await invoke<string>('ping_signaling', { url });
  return JSON.parse(raw) as PingResult;
}
