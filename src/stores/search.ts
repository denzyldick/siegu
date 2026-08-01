import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { searchFacets } from '@/services/tauri'
import type { SearchFacetsData, ActiveFilter, FacetType } from '@/types/search'

export const useSearchStore = defineStore('search', () => {
  const query = ref('')
  const recentSearches = ref<string[]>([])
  const loading = ref(false)
  const facets = ref<SearchFacetsData | null>(null)
  const facetsLoading = ref(false)
  const activeFilters = ref<ActiveFilter[]>([])
  const mediaFilters = ref({ favoritesOnly: false, videosOnly: false })

  const hasQuery = computed(() => query.value.trim().length > 0)
  const hasFilters = computed(
    () => activeFilters.value.length > 0 || mediaFilters.value.favoritesOnly || mediaFilters.value.videosOnly,
  )
  const activeFacets = computed(() => activeFilters.value)

  async function loadFacets(): Promise<void> {
    if (facetsLoading.value) return
    facetsLoading.value = true
    try {
      facets.value = await searchFacets()
    } catch (error) {
      console.error('[SearchStore] Failed to load search facets:', error)
    } finally {
      facetsLoading.value = false
    }
  }

  function setQuery(value: string): void {
    query.value = value
  }

  function clearQuery(): void {
    query.value = ''
  }

  function addFilter(filter: ActiveFilter): void {
    activeFilters.value = [
      ...activeFilters.value.filter((f) => f.type !== filter.type),
      filter,
    ]
  }

  function removeFilter(type: FacetType): void {
    activeFilters.value = activeFilters.value.filter((f) => f.type !== type)
  }

  function clearFilters(): void {
    activeFilters.value = []
    mediaFilters.value = { favoritesOnly: false, videosOnly: false }
  }

  function toggleFavoriteOnly(): void {
    mediaFilters.value.favoritesOnly = !mediaFilters.value.favoritesOnly
  }

  function toggleVideoOnly(): void {
    mediaFilters.value.videosOnly = !mediaFilters.value.videosOnly
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
    recentSearches,
    loading,
    facets,
    facetsLoading,
    activeFilters,
    activeFacets,
    mediaFilters,
    hasQuery,
    hasFilters,
    loadFacets,
    setQuery,
    clearQuery,
    addFilter,
    removeFilter,
    clearFilters,
    toggleFavoriteOnly,
    toggleVideoOnly,
    addRecentSearch,
    clearRecentSearches,
  }
})
