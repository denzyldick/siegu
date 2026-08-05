import { defineStore } from 'pinia';
import { ref } from 'vue';
import {
  listAlbums,
  createAlbum as apiCreateAlbum,
  createSmartAlbum as apiCreateSmartAlbum,
  updateSmartAlbumRule as apiUpdateSmartAlbumRule,
  clearDismissedTrips as apiClearDismissedTrips,
  renameAlbum as apiRenameAlbum,
  deleteAlbum as apiDeleteAlbum,
  addAlbumItems as apiAddAlbumItems,
  removeAlbumItems as apiRemoveAlbumItems,
  reorderAlbum as apiReorderAlbum,
  getAlbumContents as apiGetAlbumContents,
  getAlbumSections as apiGetAlbumSections,
} from '@/services/tauri';
import type { Album, AlbumSection } from '@/types/albums';
import type { MediaItem } from '@/types/media';

export const useAlbumsStore = defineStore('albums', () => {
  const albums = ref<Album[]>([]);
  const sections = ref<AlbumSection[]>([]);
  const loading = ref(false);
  const sectionsLoading = ref(false);
  const currentAlbumId = ref<string | null>(null);
  const editingSmartAlbum = ref<Album | null>(null);

  function startEditingSmartAlbum(album: Album): void {
    editingSmartAlbum.value = album;
  }

  function stopEditingSmartAlbum(): void {
    editingSmartAlbum.value = null;
  }

  async function loadAlbums(): Promise<void> {
    loading.value = true;
    try {
      albums.value = await listAlbums();
    } catch (error) {
      console.error('[AlbumsStore] Failed to load albums:', error);
    } finally {
      loading.value = false;
    }
  }

  async function loadSections(): Promise<void> {
    sectionsLoading.value = true;
    try {
      sections.value = await apiGetAlbumSections();
    } catch (error) {
      console.error('[AlbumsStore] Failed to load album sections:', error);
    } finally {
      sectionsLoading.value = false;
    }
  }

  async function createAlbum(name: string): Promise<Album | null> {
    try {
      const album = await apiCreateAlbum(name);
      albums.value.push(album);
      await loadSections();
      return album;
    } catch (error) {
      console.error('[AlbumsStore] Failed to create album:', error);
      return null;
    }
  }

  async function createSmartAlbum(
    name: string,
    rule: unknown,
    kind: 'smart' | 'trip',
  ): Promise<Album | null> {
    try {
      const album = await apiCreateSmartAlbum(name, rule, kind);
      await loadSections();
      return album;
    } catch (error) {
      console.error('[AlbumsStore] Failed to create smart album:', error);
      return null;
    }
  }

  async function updateSmartAlbumRule(albumId: string, rule: unknown): Promise<void> {
    try {
      await apiUpdateSmartAlbumRule(albumId, rule);
      await loadSections();
    } catch (error) {
      console.error('[AlbumsStore] Failed to update smart album rule:', error);
    }
  }

  async function renameAlbum(albumId: string, name: string): Promise<void> {
    try {
      await apiRenameAlbum(albumId, name);
      const album = albums.value.find((a) => a.id === albumId);
      if (album) album.name = name;
      await loadSections();
    } catch (error) {
      console.error('[AlbumsStore] Failed to rename album:', error);
    }
  }

  async function deleteAlbum(albumId: string): Promise<void> {
    try {
      await apiDeleteAlbum(albumId);
      albums.value = albums.value.filter((a) => a.id !== albumId);
      if (currentAlbumId.value === albumId) currentAlbumId.value = null;
      await loadSections();
    } catch (error) {
      console.error('[AlbumsStore] Failed to delete album:', error);
    }
  }

  async function addItems(albumId: string, photoIds: string[]): Promise<void> {
    try {
      await apiAddAlbumItems(albumId, photoIds);
      await loadAlbums();
    } catch (error) {
      console.error('[AlbumsStore] Failed to add items to album:', error);
    }
  }

  async function removeItems(albumId: string, photoIds: string[]): Promise<void> {
    try {
      await apiRemoveAlbumItems(albumId, photoIds);
      await loadAlbums();
    } catch (error) {
      console.error('[AlbumsStore] Failed to remove items from album:', error);
    }
  }

  async function reorderItems(albumId: string, orderedIds: string[]): Promise<void> {
    try {
      await apiReorderAlbum(albumId, orderedIds);
    } catch (error) {
      console.error('[AlbumsStore] Failed to reorder album:', error);
    }
  }

  async function loadContents(
    albumId: string,
    offset: number,
    limit: number,
  ): Promise<MediaItem[]> {
    try {
      return await apiGetAlbumContents(albumId, offset, limit);
    } catch (error) {
      console.error('[AlbumsStore] Failed to load album contents:', error);
      return [];
    }
  }

  async function clearDismissedTrips(): Promise<void> {
    try {
      await apiClearDismissedTrips();
      await loadSections();
    } catch (error) {
      console.error('[AlbumsStore] Failed to clear dismissed trips:', error);
    }
  }

  return {
    albums,
    sections,
    loading,
    sectionsLoading,
    currentAlbumId,
    editingSmartAlbum,
    startEditingSmartAlbum,
    stopEditingSmartAlbum,
    loadAlbums,
    loadSections,
    createAlbum,
    createSmartAlbum,
    updateSmartAlbumRule,
    clearDismissedTrips,
    renameAlbum,
    deleteAlbum,
    addItems,
    removeItems,
    reorderItems,
    loadContents,
  };
});
