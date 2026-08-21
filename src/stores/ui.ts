import { defineStore } from 'pinia';
import { ref } from 'vue';

export type PageId = 'home' | 'collections' | 'location' | 'devices' | 'settings';

export const useUiStore = defineStore('ui', () => {
  const currentPage = ref<PageId>('home');
  const theme = ref<string>(localStorage.getItem('siegu_theme') || 'system');
  const language = ref<string>(localStorage.getItem('siegu_language') || 'en');
  const sidebarOpen = ref(false);
  const viewerOpen = ref(false);
  const viewerMediaId = ref<number | null>(null);

  function setPage(page: PageId): void {
    currentPage.value = page;
  }

  function setTheme(newTheme: string): void {
    theme.value = newTheme;
    localStorage.setItem('siegu_theme', newTheme);
  }

  function setLanguage(lang: string): void {
    language.value = lang;
    localStorage.setItem('siegu_language', lang);
  }

  function toggleSidebar(): void {
    sidebarOpen.value = !sidebarOpen.value;
  }

  function openViewer(mediaId: number): void {
    viewerMediaId.value = mediaId;
    viewerOpen.value = true;
  }

  function closeViewer(): void {
    viewerOpen.value = false;
    viewerMediaId.value = null;
  }

  return {
    currentPage,
    theme,
    language,
    sidebarOpen,
    viewerOpen,
    viewerMediaId,
    setPage,
    setTheme,
    setLanguage,
    toggleSidebar,
    openViewer,
    closeViewer,
  };
});
