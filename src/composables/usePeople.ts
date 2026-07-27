import { ref, onMounted, onBeforeUnmount } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import type { Person } from '@/types/person'
import {
  getPeople,
  getUnnamedFaces,
  getPersonFaces,
  indexFaces,
  getIndexingStatus,
  assignNameToFace,
  renamePerson,
  mergePeople,
  deleteFace,
} from '@/services/tauri'
import { getFaceImageSrc, normalizeIndexingCount } from '@/composables/useMediaUtils'

export function usePeople() {
  const people = ref<Person[]>([])
  const unnamedFaces = ref<Person[]>([])
  const indexingCount = ref(0)
  const unlistenProgress = ref<UnlistenFn | null>(null)

  async function fetchData(): Promise<void> {
    try {
      people.value = await getPeople()
      unnamedFaces.value = await getUnnamedFaces()
    } catch (e) {
      console.error('Failed to fetch people data:', e)
    }
  }

  async function startIndexing(): Promise<void> {
    try {
      await indexFaces()
    } catch (e) {
      console.error('Failed to start indexing:', e)
    }
  }

  async function saveName(faceId: number, name: string): Promise<void> {
    try {
      await assignNameToFace(faceId, name)
      await fetchData()
    } catch (e) {
      console.error('Failed to assign name:', e)
    }
  }

  async function renamePersonById(id: number, newName: string): Promise<void> {
    try {
      await renamePerson(id, newName)
      await fetchData()
    } catch (e) {
      console.error('Failed to rename person:', e)
    }
  }

  async function mergePersonById(fromId: number, toId: number): Promise<void> {
    try {
      await mergePeople(fromId, toId)
      await fetchData()
    } catch (e) {
      console.error('Failed to merge people:', e)
    }
  }

  async function fetchClusterFaces(personId: number) {
    try {
      return await getPersonFaces(personId)
    } catch (e) {
      console.error('Failed to fetch cluster faces:', e)
      return []
    }
  }

  async function removeFace(faceId: number): Promise<boolean> {
    try {
      await deleteFace(faceId)
      return true
    } catch (e) {
      console.error('Failed to remove face:', e)
      return false
    }
  }

  function formatIndexingCount(value: number): string {
    return normalizeIndexingCount(value).toLocaleString(
      localStorage.getItem('siegu_language') || 'en',
    )
  }

  onMounted(async () => {
    await fetchData()
    const count = await getIndexingStatus()
    indexingCount.value = normalizeIndexingCount(count)

    unlistenProgress.value = await listen('indexing-progress', (event) => {
      indexingCount.value = normalizeIndexingCount(event.payload)
      if (indexingCount.value === 0) {
        fetchData()
      }
    })
  })

  onBeforeUnmount(() => {
    unlistenProgress.value?.()
  })

  return {
    people,
    unnamedFaces,
    indexingCount,
    fetchData,
    startIndexing,
    saveName,
    renamePersonById,
    mergePersonById,
    fetchClusterFaces,
    removeFace,
    formatIndexingCount,
    getFaceImageSrc,
  }
}
