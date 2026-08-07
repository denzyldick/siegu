import { describe, it, expect, beforeEach, vi } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';

vi.mock('@/services/tauri', () => ({
  listDirectories: vi.fn(),
}));

import { listDirectories } from '@/services/tauri';
import { useAppStore } from '@/stores/app';
import { useUiStore } from '@/stores/ui';

describe('critical path: settings load', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    localStorage.clear();
    vi.clearAllMocks();
  });

  it('loadDirectories populates directories', async () => {
    vi.mocked(listDirectories).mockResolvedValue(['/home/photos', '/home/videos'] as never);

    const store = useAppStore();
    await store.loadDirectories();

    expect(store.directories).toEqual(['/home/photos', '/home/videos']);
  });

  it('loadDirectories handles errors gracefully', async () => {
    vi.mocked(listDirectories).mockRejectedValue(new Error('failed'));

    const store = useAppStore();
    await store.loadDirectories();

    expect(store.directories).toEqual([]);
  });

  it('theme persists in localStorage', () => {
    const store = useUiStore();
    store.setTheme('dark');
    expect(localStorage.getItem('siegu_theme')).toBe('dark');
  });

  it('language persists in localStorage', () => {
    const store = useUiStore();
    store.setLanguage('nl');
    expect(localStorage.getItem('siegu_language')).toBe('nl');
  });
});
