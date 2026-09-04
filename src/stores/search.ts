import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { searchFacets, resolvePhotoLocations } from '@/services/tauri';
import type { SearchFacetsData, ActiveFilter, FacetType } from '@/types/search';

export const useSearchStore = defineStore('search', () => {
  const query = ref('');
  const recentSearches = ref<string[]>([]);
  const loading = ref(false);
  const facets = ref<SearchFacetsData | null>(null);
  const facetsLoading = ref(false);
  const activeFilters = ref<ActiveFilter[]>([]);
  const mediaFilters = ref({
    favoritesOnly: false,
    videosOnly: false,
    facesOnly: false,
    papersOnly: false,
    nsfwOnly: false,
    storedOnly: false,
    notStoredOnly: false,
  });
  const camera = ref<string | null>(null);
  const aestheticsMin = ref<number | null>(null);
  const surprise = ref(false);
  const sortOrder = ref<'newest' | 'oldest' | 'best' | 'random'>('newest');
  const dateRange = ref<[string, string] | null>(null);
  const personMatch = ref<'and' | 'or'>('and');
  const personAlone = ref(false);
  let locationsResolved = false;

  const hasQuery = computed(() => query.value.trim().length > 0);
  const hasFilters = computed(
    () =>
      activeFilters.value.length > 0 ||
      mediaFilters.value.favoritesOnly ||
      mediaFilters.value.videosOnly ||
      mediaFilters.value.facesOnly ||
      mediaFilters.value.papersOnly ||
      mediaFilters.value.nsfwOnly ||
      mediaFilters.value.storedOnly ||
      mediaFilters.value.notStoredOnly ||
      camera.value !== null ||
      aestheticsMin.value !== null ||
      dateRange.value !== null,
  );
  const activeFacets = computed(() => activeFilters.value);
  const personCount = computed(() => activeFilters.value.filter((f) => f.type === 'person').length);

  async function loadFacets(): Promise<void> {
    if (facetsLoading.value) return;
    if (!locationsResolved) {
      locationsResolved = true;
      resolvePhotoLocations()
        .catch((error) => {
          console.error('[SearchStore] Failed to resolve photo locations:', error);
        })
        .finally(() => {
          if (facets.value) void loadFacets();
        });
    }
    facetsLoading.value = true;
    try {
      facets.value = await searchFacets();
    } catch (error) {
      console.error('[SearchStore] Failed to load search facets:', error);
    } finally {
      facetsLoading.value = false;
    }
  }

  function setQuery(value: string): void {
    query.value = value;
  }

  function clearQuery(): void {
    query.value = '';
  }

  function addFilter(filter: ActiveFilter): void {
    if (filter.type === 'person') {
      const exists = activeFilters.value.some(
        (f) => f.type === 'person' && f.value === filter.value,
      );
      if (exists) return;
      activeFilters.value = [...activeFilters.value, filter];
      return;
    }
    activeFilters.value = [...activeFilters.value.filter((f) => f.type !== filter.type), filter];
  }

  function removeFilter(type: FacetType): void {
    activeFilters.value = activeFilters.value.filter((f) => f.type !== type);
    if (type === 'person' && !activeFilters.value.some((f) => f.type === 'person')) {
      personMatch.value = 'and';
      personAlone.value = false;
    }
  }

  function removeFilterValue(type: FacetType, value: string): void {
    activeFilters.value = activeFilters.value.filter(
      (f) => !(f.type === type && f.value === value),
    );
    if (type === 'person' && !activeFilters.value.some((f) => f.type === 'person')) {
      personMatch.value = 'and';
      personAlone.value = false;
    }
  }

  function togglePerson(person: { id: string; name: string }): void {
    const exists = activeFilters.value.find((f) => f.type === 'person' && f.value === person.id);
    if (exists) removeFilterValue('person', person.id);
    else addFilter({ type: 'person', value: person.id, label: person.name });
  }

  function setPersonMatch(match: 'and' | 'or'): void {
    personMatch.value = match;
  }

  function togglePersonAlone(): void {
    personAlone.value = !personAlone.value;
  }

  function setCamera(value: string | null): void {
    camera.value = value;
  }

  function setAestheticsMin(value: number | null): void {
    aestheticsMin.value = value;
  }

  function setDateRange(value: [string, string] | null): void {
    dateRange.value = value;
  }

  function toggleSurprise(): void {
    surprise.value = true;
  }

  function clearSurprise(): void {
    surprise.value = false;
  }

  function setSortOrder(order: 'newest' | 'oldest' | 'best' | 'random'): void {
    sortOrder.value = order;
    if (order === 'random') {
      surprise.value = true;
    } else if (surprise.value) {
      surprise.value = false;
    }
  }

  function clearFilters(): void {
    activeFilters.value = [];
    mediaFilters.value = {
      favoritesOnly: false,
      videosOnly: false,
      facesOnly: false,
      papersOnly: false,
      nsfwOnly: false,
      storedOnly: false,
      notStoredOnly: false,
    };
    camera.value = null;
    aestheticsMin.value = null;
    surprise.value = false;
    sortOrder.value = 'newest';
    dateRange.value = null;
    personMatch.value = 'and';
    personAlone.value = false;
  }

  function toggleFavoriteOnly(): void {
    mediaFilters.value.favoritesOnly = !mediaFilters.value.favoritesOnly;
  }

  function toggleVideoOnly(): void {
    mediaFilters.value.videosOnly = !mediaFilters.value.videosOnly;
  }

  function toggleFacesOnly(): void {
    mediaFilters.value.facesOnly = !mediaFilters.value.facesOnly;
  }

  function togglePapersOnly(): void {
    mediaFilters.value.papersOnly = !mediaFilters.value.papersOnly;
  }

  function toggleNsfwOnly(): void {
    mediaFilters.value.nsfwOnly = !mediaFilters.value.nsfwOnly;
  }

  /**
   * Cycle the storage availability filter: All -> Stored only -> Not stored
   * only -> All. Only one direction is active at a time because they are
   * mutually exclusive.
   */
  function cycleStorageFilter(): void {
    const { storedOnly, notStoredOnly } = mediaFilters.value;
    if (!storedOnly && !notStoredOnly) {
      mediaFilters.value.storedOnly = true;
    } else if (storedOnly) {
      mediaFilters.value.storedOnly = false;
      mediaFilters.value.notStoredOnly = true;
    } else {
      mediaFilters.value.notStoredOnly = false;
    }
  }

  function addRecentSearch(term: string): void {
    if (!term.trim()) return;
    recentSearches.value = [term, ...recentSearches.value.filter((s) => s !== term)].slice(0, 10);
    saveRecentSearches();
  }

  async function applyRule(rule: Record<string, unknown>): Promise<void> {
    clearFilters();
    setQuery(String(rule.query ?? ''));
    const rawPeople = rule.person_ids;
    if (Array.isArray(rawPeople) && rawPeople.length > 0) {
      if (!facets.value) await loadFacets();
      const people = (facets.value?.people ?? []) as { id: string; name: string | null }[];
      const nameFor = new Map(people.map((p) => [p.id, p.name ?? p.id]));
      for (const id of rawPeople as string[]) {
        addFilter({ type: 'person', value: id, label: nameFor.get(id) ?? id });
      }
    }
    if (rule.person_match === 'or') personMatch.value = 'or';
    if (rule.person_alone) personAlone.value = true;
    if (rule.location) {
      const value = String(rule.location);
      addFilter({ type: 'location', value, label: value });
    }
    if (rule.tag) {
      const value = String(rule.tag);
      addFilter({ type: 'tag', value, label: value });
    }
    if (rule.date_from && rule.date_to) {
      const fromDate = String(rule.date_from).slice(0, 10);
      const toDate = String(rule.date_to).slice(0, 10);
      addFilter({
        type: 'date',
        value: `${fromDate}|${toDate}`,
        label: fromDate === toDate ? fromDate : `${fromDate} → ${toDate}`,
      });
    }
    if (rule.favorite) mediaFilters.value.favoritesOnly = true;
    if (rule.videos) mediaFilters.value.videosOnly = true;
    if (rule.has_faces) mediaFilters.value.facesOnly = true;
    if (rule.papers) mediaFilters.value.papersOnly = true;
    if (rule.nsfw_only) mediaFilters.value.nsfwOnly = true;
    if (rule.camera) camera.value = String(rule.camera);
    if (rule.aesthetics_min != null) aestheticsMin.value = Number(rule.aesthetics_min);
    if (rule.order_by && rule.order_by !== 'newest') {
      setSortOrder(rule.order_by as 'newest' | 'oldest' | 'best' | 'random');
    }
  }

  function clearRecentSearches(): void {
    recentSearches.value = [];
    saveRecentSearches();
  }

  function saveRecentSearches(): void {
    localStorage.setItem('siegu_recent_searches', JSON.stringify(recentSearches.value));
  }

  function loadRecentSearches(): void {
    try {
      const stored = localStorage.getItem('siegu_recent_searches');
      if (stored) {
        recentSearches.value = JSON.parse(stored) as string[];
      }
    } catch {
      recentSearches.value = [];
    }
  }

  loadRecentSearches();

  return {
    query,
    recentSearches,
    loading,
    facets,
    facetsLoading,
    activeFilters,
    activeFacets,
    personCount,
    mediaFilters,
    camera,
    aestheticsMin,
    surprise,
    sortOrder,
    dateRange,
    personMatch,
    personAlone,
    hasQuery,
    hasFilters,
    loadFacets,
    setQuery,
    clearQuery,
    addFilter,
    removeFilter,
    removeFilterValue,
    togglePerson,
    setPersonMatch,
    togglePersonAlone,
    setCamera,
    setAestheticsMin,
    setDateRange,
    setSortOrder,
    toggleSurprise,
    clearSurprise,
    clearFilters,
    toggleFavoriteOnly,
    toggleVideoOnly,
    toggleFacesOnly,
    togglePapersOnly,
    toggleNsfwOnly,
    cycleStorageFilter,
    addRecentSearch,
    applyRule,
    clearRecentSearches,
  };
});
