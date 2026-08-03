import { defineStore } from 'pinia'
import { ref } from 'vue'
import {
  listAlbums,
  createAlbum as apiCreateAlbum,
  renameAlbum as apiRenameAlbum,
  deleteAlbum as apiDeleteAlbum,
  addAlbumItems as apiAddAlbumItems,
  removeAlbumItems as apiRemoveAlbumItems,
  reorderAlbum as apiReorderAlbum,
  getAlbumContents as apiGetAlbumContents,
} from '@/services/tauri'
import type { Album } from '@/types/albums'
import type { MediaItem } from '@/types/media'

export const useAlbumsStore = defineStore('albums', () => {
  const albums = ref<Album[]>([])
  const loading = ref(false)
  const currentAlbumId = ref<string | null>(null)

  async function loadAlbums(): Promise<void> {
    loading.value = true
    try {
      albums.value = await listAlbums()
    } catch (error) {
      console.error('[AlbumsStore] Failed to load albums:', error)
    } finally {
      loading.value = false
    }
  }

  async function createAlbum(name: string): Promise<Album | null> {
    try {
      const album = await apiCreateAlbum(name)
      albums.value.push(album)
      return album
    } catch (error) {
      console.error('[AlbumsStore] Failed to create album:', error)
      return null
    }
  }

  async function renameAlbum(albumId: string, name: string): Promise<void> {
    try {
      await apiRenameAlbum(albumId, name)
      const album = albums.value.find((a) => a.id === albumId)
      if (album) album.name = name
    } catch (error) {
      console.error('[AlbumsStore] Failed to rename album:', error)
    }
  }

  async function deleteAlbum(albumId: string): Promise<void> {
    try {
      await apiDeleteAlbum(albumId)
      albums.value = albums.value.filter((a) => a.id !== albumId)
      if (currentAlbumId.value === albumId) currentAlbumId.value = null
    } catch (error) {
      console.error('[AlbumsStore] Failed to delete album:', error)
    }
  }

  async function addItems(albumId: string, photoIds: string[]): Promise<void> {
    try {
      await apiAddAlbumItems(albumId, photoIds)
      await loadAlbums()
    } catch (error) {
      console.error('[AlbumsStore] Failed to add items to album:', error)
    }
  }

  async function removeItems(albumId: string, photoIds: string[]): Promise<void> {
    try {
      await apiRemoveAlbumItems(albumId, photoIds)
      await loadAlbums()
    } catch (error) {
      console.error('[AlbumsStore] Failed to remove items from album:', error)
    }
  }

  async function reorderItems(albumId: string, orderedIds: string[]): Promise<void> {
    try {
      await apiReorderAlbum(albumId, orderedIds)
    } catch (error) {
      console.error('[AlbumsStore] Failed to reorder album:', error)
    }
  }

  async function loadContents(albumId: string, offset: number, limit: number): Promise<MediaItem[]> {
    try {
      return await apiGetAlbumContents(albumId, offset, limit)
    } catch (error) {
      console.error('[AlbumsStore] Failed to load album contents:', error)
      return []
    }
  }

  return {
    albums,
    loading,
    currentAlbumId,
    loadAlbums,
    createAlbum,
    renameAlbum,
    deleteAlbum,
    addItems,
    removeItems,
    reorderItems,
    loadContents,
  }
})
