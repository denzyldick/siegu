import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { getTopTags, listObjects } from '@/services/tauri'

export interface SearchTag {
  title: string
  type: string
}

export const useSearchStore = defineStore('search', () => {
  const query = ref('')
  const tags = ref<SearchTag[]>([])
  const recentSearches = ref<string[]>([])
  const loading = ref(false)

  const hasQuery = computed(() => query.value.trim().length > 0)

  async function loadTopTags(): Promise<void> {
    try {
      tags.value = await getTopTags()
    } catch (error) {
      console.error('[SearchStore] Failed to load top tags:', error)
      tags.value = []
    }
  }

  async function searchObjects(q: string): Promise<SearchTag[]> {
    if (!q.trim()) return []
    try {
      return await listObjects(q)
    } catch (error) {
      console.error('[SearchStore] Failed to search objects:', error)
      return []
    }
  }

  function setQuery(value: string): void {
    query.value = value
  }

  function clearQuery(): void {
    query.value = ''
  }

  function addRecentSearch(term: string): void {
    if (!term.trim()) return
    recentSearches.value = [
      term,
      ...recentSearches.value.filter((s) => s !== term),
    ].slice(0, 10)
    saveRecentSearches()
  }

  function clearRecentSearches(): void {
    recentSearches.value = []
    saveRecentSearches()
  }

  function saveRecentSearches(): void {
    localStorage.setItem('siegu_recent_searches', JSON.stringify(recentSearches.value))
  }

  function loadRecentSearches(): void {
    try {
      const stored = localStorage.getItem('siegu_recent_searches')
      if (stored) {
        recentSearches.value = JSON.parse(stored) as string[]
      }
    } catch {
      recentSearches.value = []
    }
  }

  loadRecentSearches()

  return {
    query,
    tags,
    recentSearches,
    loading,
    hasQuery,
    loadTopTags,
    searchObjects,
    setQuery,
    clearQuery,
    addRecentSearch,
    clearRecentSearches,
  }
})
