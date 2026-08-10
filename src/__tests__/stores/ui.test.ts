import { describe, it, expect, beforeEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useUiStore } from '@/stores/ui';

describe('ui store', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    localStorage.clear();
  });

  it('defaults to home page', () => {
    const store = useUiStore();
    expect(store.currentPage).toBe('home');
  });

  it('sets current page', () => {
    const store = useUiStore();
    store.setPage('location');
    expect(store.currentPage).toBe('location');
    expect(localStorage.getItem('siegu_page')).toBeNull();
  });

  it('sets theme and persists', () => {
    const store = useUiStore();
    store.setTheme('dark');
    expect(store.theme).toBe('dark');
    expect(localStorage.getItem('siegu_theme')).toBe('dark');
  });

  it('sets language and persists', () => {
    const store = useUiStore();
    store.setLanguage('nl');
    expect(store.language).toBe('nl');
    expect(localStorage.getItem('siegu_language')).toBe('nl');
  });

  it('opens and closes viewer', () => {
    const store = useUiStore();
    expect(store.viewerOpen).toBe(false);

    store.openViewer(42);
    expect(store.viewerOpen).toBe(true);
    expect(store.viewerMediaId).toBe(42);

    store.closeViewer();
    expect(store.viewerOpen).toBe(false);
    expect(store.viewerMediaId).toBe(null);
  });
});
