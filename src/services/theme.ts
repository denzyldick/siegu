import { invoke } from '@tauri-apps/api/core';
import { platform } from '@tauri-apps/plugin-os';

let systemDarkOverride: boolean | null = null;

export function getThemePreference(): string {
  return localStorage.getItem('siegu_theme') || 'system';
}

export function resolveTheme(): string {
  const pref = getThemePreference();
  if (pref === 'light') return 'light';
  if (pref === 'dark') return 'dark';
  if (systemDarkOverride !== null) return systemDarkOverride ? 'dark' : 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function syncTheme(apply: (resolved: string) => void): void {
  const resolved = resolveTheme();
  document.documentElement.dataset.theme = resolved;
  document.documentElement.style.colorScheme = resolved;
  apply(resolved);
}

export async function initSystemTheme(apply: (resolved: string) => void): Promise<void> {
  try {
    if (platform() !== 'linux') return;
    const dark = await invoke<boolean | null>('get_system_dark_mode');
    if (typeof dark !== 'boolean') return;
    systemDarkOverride = dark;
    if (getThemePreference() === 'system') {
      syncTheme(apply);
    }
  } catch {
    // Fall back to prefers-color-scheme; the app keeps working in the browser.
  }
}
