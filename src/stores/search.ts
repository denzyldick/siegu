import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { searchFacets, resolvePhotoLocations } from '@/services/tauri'
import type { SearchFacetsData, ActiveFilter, FacetType } from '@/types/search'

export const useSearchStore = defineStore('search', () => {
  const query = ref('')
  const recentSearches = ref<string[]>([])
  const loading = ref(false)
  const facets = ref<SearchFacetsData | null>(null)
  const facetsLoading = ref(false)
  const activeFilters = ref<ActiveFilter[]>([])
  const mediaFilters = ref({
    favoritesOnly: false,
    videosOnly: false,
    facesOnly: false,
    papersOnly: false,
  })
  const camera = ref<string | null>(null)
  const aestheticsMin = ref<number | null>(null)
  const surprise = ref(false)
  const dateRange = ref<[string, string] | null>(null)
  let locationsResolved = false

  const hasQuery = computed(() => query.value.trim().length > 0)
  const hasFilters = computed(
    () =>
      activeFilters.value.length > 0 ||
      mediaFilters.value.favoritesOnly ||
      mediaFilters.value.videosOnly ||
      mediaFilters.value.facesOnly ||
      mediaFilters.value.papersOnly ||
      camera.value !== null ||
      aestheticsMin.value !== null ||
      dateRange.value !== null,
  )
  const activeFacets = computed(() => activeFilters.value)

  async function loadFacets(): Promise<void> {
    if (facetsLoading.value) return
    if (!locationsResolved) {
      locationsResolved = true
      resolvePhotoLocations()
        .catch((error) => {
          console.error('[SearchStore] Failed to resolve photo locations:', error)
        })
        .finally(() => {
          if (facets.value) void loadFacets()
        })
    }
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

  function setCamera(value: string | null): void {
    camera.value = value
  }

  function setAestheticsMin(value: number | null): void {
    aestheticsMin.value = value
  }

  function setDateRange(value: [string, string] | null): void {
    dateRange.value = value
  }

  function toggleSurprise(): void {
    surprise.value = true
  }

  function clearSurprise(): void {
    surprise.value = false
  }

  function clearFilters(): void {
    activeFilters.value = []
    mediaFilters.value = {
      favoritesOnly: false,
      videosOnly: false,
      facesOnly: false,
      papersOnly: false,
    }
    camera.value = null
    aestheticsMin.value = null
    surprise.value = false
    dateRange.value = null
  }

  function toggleFavoriteOnly(): void {
    mediaFilters.value.favoritesOnly = !mediaFilters.value.favoritesOnly
  }

  function toggleVideoOnly(): void {
    mediaFilters.value.videosOnly = !mediaFilters.value.videosOnly
  }

  function toggleFacesOnly(): void {
    mediaFilters.value.facesOnly = !mediaFilters.value.facesOnly
  }

  function togglePapersOnly(): void {
    mediaFilters.value.papersOnly = !mediaFilters.value.papersOnly
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
    camera,
    aestheticsMin,
    surprise,
    dateRange,
    hasQuery,
    hasFilters,
    loadFacets,
    setQuery,
    clearQuery,
    addFilter,
    removeFilter,
    setCamera,
    setAestheticsMin,
    setDateRange,
    toggleSurprise,
    clearSurprise,
    clearFilters,
    toggleFavoriteOnly,
    toggleVideoOnly,
    toggleFacesOnly,
    togglePapersOnly,
    addRecentSearch,
    clearRecentSearches,
  }
})
