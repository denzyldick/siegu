<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSearchStore } from '@/stores/search'
import {
  getFaceImageSrc,
  getMediaThumbnailSrc,
} from '@/composables/useMediaUtils'
import { listFiles } from '@/services/tauri'
import DateRangePicker from '@/components/search/DateRangePicker.vue'
import type { FacetGroup, LocationGroup, PhotoTile } from '@/types/search'
import type { MediaItem } from '@/types/media'

const emit = defineEmits<{
  (e: 'search', query: string): void
  (e: 'advanced'): void
}>()

const { t } = useI18n()
const searchStore = useSearchStore()

const searchWrapRef = ref<HTMLElement | null>(null)
const dropdownOpen = ref(false)
const dropdownStyle = ref<Record<string, string>>({})
const inlineResults = ref<MediaItem[]>([])
const inlineLoading = ref(false)
let searchTimer: ReturnType<typeof setTimeout> | null = null
let fetchTimer: ReturnType<typeof setTimeout> | null = null

const facets = computed(() => searchStore.facets)
const stats = computed(() => facets.value?.stats ?? null)

const q = computed(() => searchStore.query.trim().toLowerCase())

function matches(value: string): boolean {
  return value.toLowerCase().includes(q.value)
}

const namedPeople = computed(() =>
  (facets.value?.people ?? []).filter((p) => !q.value || matches(p.name ?? '')),
)

const peopleRow = computed(() =>
  q.value
    ? namedPeople.value.slice(0, 12)
    : namedPeople.value.slice(0, 14),
)

const locations = computed(() =>
  (facets.value?.locations ?? []).filter((l) => !q.value || matches(l.name)),
)

const tags = computed(() => (facets.value?.tags ?? []).filter((l) => !q.value || matches(l.name)))

const papers = computed(() =>
  (facets.value?.papers ?? []).filter((p) => !q.value || matches(p.name)),
)

const cameras = computed(() =>
  (facets.value?.cameras ?? []).filter((c) => !q.value || matches(c.name)),
)

const dateRange = computed(() => searchStore.dateRange)

const bestPhotos = computed(() => (facets.value?.best_photos ?? []).slice(0, 10))

const recentSearches = computed(() => searchStore.recentSearches)

const isMobile = computed(() => window.innerWidth < 640)

function faceSrc(person: FacetGroup): string {
  return getFaceImageSrc(person.representative_crop, person.encoded)
}

function tileSrc(tile: PhotoTile): string {
  return getMediaThumbnailSrc(tile.location, tile.encoded, true)
}

function locationSrc(group: LocationGroup): string {
  return getMediaThumbnailSrc(group.photo_location ?? '', group.encoded ?? '', true)
}

function repositionDropdown(): void {
  const rect = searchWrapRef.value?.getBoundingClientRect()
  if (!rect) return
  const viewportH = window.innerHeight
  const flipUp = rect.bottom + 480 > viewportH && rect.top > viewportH - rect.bottom
  const styles: Record<string, string> = {
    position: 'fixed',
    zIndex: '5000',
  }
  if (isMobile.value) {
    styles.left = '8px'
    styles.right = '8px'
  } else {
    styles.width = `${Math.min(660, Math.max(520, rect.width))}px`
    styles.left = `${Math.max(8, rect.left + rect.width / 2 - 330)}px`
  }
  if (flipUp) {
    styles.bottom = `${viewportH - rect.top + 8}px`
    styles.top = 'auto'
  } else {
    styles.top = `${rect.bottom + 8}px`
    styles.bottom = 'auto'
  }
  dropdownStyle.value = styles
}

function openDropdown(): void {
  dropdownOpen.value = true
  repositionDropdown()
  if (!searchStore.facets) {
    searchStore.loadFacets()
  }
}

function closeDropdown(): void {
  dropdownOpen.value = false
}

function selectPerson(person: FacetGroup): void {
  searchStore.addFilter({ type: 'person', value: person.id, label: person.name ?? '' })
  closeDropdown()
}

function selectLocation(name: string): void {
  searchStore.addFilter({ type: 'location', value: name, label: name })
  closeDropdown()
}

function selectTag(name: string): void {
  searchStore.addFilter({ type: 'tag', value: name, label: name })
  closeDropdown()
}

function selectPaper(name: string): void {
  searchStore.addFilter({ type: 'tag', value: name, label: labelFromPaper(name) })
  closeDropdown()
}

function selectCamera(name: string): void {
  searchStore.setCamera(name)
  closeDropdown()
}

function formatDateLabel(range: [string, string]): string {
  const locale = localStorage.getItem('siegu_language') || 'en'
  const fmt = (d: string) =>
    new Date(`${d}T00:00:00`).toLocaleDateString(locale, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    })
  return range[0] === range[1] ? fmt(range[0]) : `${fmt(range[0])} — ${fmt(range[1])}`
}

function onDateRangeChange(range: [string, string] | null): void {
  searchStore.setDateRange(range)
  if (range) {
    searchStore.addFilter({ type: 'date', value: `${range[0]}|${range[1]}`, label: formatDateLabel(range) })
  } else {
    searchStore.removeFilter('date')
  }
  if (range) closeDropdown()
}

function toggleMedia(type: 'favorites' | 'videos' | 'faces' | 'papers'): void {
  if (type === 'favorites') searchStore.toggleFavoriteOnly()
  if (type === 'videos') searchStore.toggleVideoOnly()
  if (type === 'faces') searchStore.toggleFacesOnly()
  if (type === 'papers') searchStore.togglePapersOnly()
  closeDropdown()
}

function surpriseMe(): void {
  searchStore.clearFilters()
  searchStore.toggleSurprise()
  closeDropdown()
}

function labelFromPaper(name: string): string {
  const map: Record<string, string> = {
    'a passport': t('search.papers.passport'),
    "a driver's license": t('search.papers.license'),
    'an id card': t('search.papers.id_card'),
    'a document': t('search.papers.document'),
    'a receipt': t('search.papers.receipt'),
    'a screenshot': t('search.papers.screenshot'),
    'a meme': t('search.papers.meme'),
    'a text message': t('search.papers.text_message'),
  }
  return map[name] ?? name
}

function runSearch(): void {
  const term = searchStore.query.trim()
  if (!term) return
  searchStore.addRecentSearch(term)
  emit('search', term)
  closeDropdown()
}

function selectRecent(term: string): void {
  searchStore.setQuery(term)
  emit('search', term)
  closeDropdown()
}

function clearAll(): void {
  searchStore.clearQuery()
  searchStore.clearFilters()
  closeDropdown()
}

function isActive(type: string, value: string): boolean {
  return searchStore.activeFilters.some((f) => f.type === type && f.value === value)
}

function isMediaActive(key: 'favoritesOnly' | 'videosOnly' | 'facesOnly' | 'papersOnly'): boolean {
  return searchStore.mediaFilters[key]
}

function activeCount(type: string): number {
  const counts: Record<string, string | number> = {
    favorites: stats.value?.favorites ?? 0,
    videos: stats.value?.videos ?? 0,
    faces: stats.value?.face_photos ?? 0,
    papers: (facets.value?.papers ?? []).reduce((sum, p) => sum + p.count, 0),
  }
  return counts[type] as number
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter') {
    runSearch()
  } else if (event.key === 'Escape') {
    closeDropdown()
  }
}

function onDocumentClick(event: MouseEvent): void {
  if (!dropdownOpen.value) return
  const target = event.target as Node
  if (searchWrapRef.value && !searchWrapRef.value.contains(target)) {
    closeDropdown()
  }
}

function onScroll(): void {
  if (dropdownOpen.value) {
    repositionDropdown()
  }
}

watch(q, () => {
  if (!dropdownOpen.value) return
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => {
    if (fetchTimer) clearTimeout(fetchTimer)
    fetchTimer = setTimeout(async () => {
      const term = searchStore.query.trim()
      if (!term) {
        inlineResults.value = []
        return
      }
      inlineLoading.value = true
      try {
        const results = await listFiles({ offset: 0, limit: 12, query: term })
        inlineResults.value = results
      } catch (e) {
        console.error('[SearchBar] inline results failed:', e)
      } finally {
        inlineLoading.value = false
      }
    }, 180)
  }, 120)
})

onMounted(() => {
  searchStore.loadFacets()
  document.addEventListener('click', onDocumentClick)
  window.addEventListener('resize', onScroll)
  window.addEventListener('scroll', onScroll, true)
})

onUnmounted(() => {
  document.removeEventListener('click', onDocumentClick)
  window.removeEventListener('resize', onScroll)
  window.removeEventListener('scroll', onScroll, true)
  if (searchTimer) clearTimeout(searchTimer)
  if (fetchTimer) clearTimeout(fetchTimer)
})

function iconForFilter(type: string): string {
  return (
    {
      person: 'mdi-account',
      location: 'mdi-map-marker',
      tag: 'mdi-tag',
      month: 'mdi-calendar-month',
      camera: 'mdi-camera',
      aesthetics: 'mdi-star-four-points',
    }[type] ?? 'mdi-filter-variant'
  )
}
</script>

<template>
  <div ref="searchWrapRef" class="search-wrapper">
    <div class="search-field" data-tour="search" @click="openDropdown">
      <v-icon size="20" class="search-icon" color="#a1a1aa">mdi-magnify</v-icon>
      <input
        v-model="searchStore.query"
        class="search-input"
        :placeholder="t('search.placeholder')"
        @focus="openDropdown"
        @keydown="onKeydown"
      />
      <v-icon
        v-if="searchStore.query || searchStore.hasFilters"
        size="18"
        color="#a1a1aa"
        class="cursor-pointer"
        @click.stop="clearAll"
      >
        mdi-close-circle
      </v-icon>
    </div>

    <Teleport to="body">
      <div v-if="dropdownOpen" class="search-dropdown" :style="dropdownStyle">
        <div v-if="!facets && !searchStore.facetsLoading" class="pa-6 d-flex justify-center">
          <v-progress-circular indeterminate size="24" width="2" />
        </div>

        <template v-else>
          <!-- ============ BROWSE MODE ============ -->
          <template v-if="!q">
            <div class="discover-header">
              <div>
                <div class="text-overline font-weight-black text-zinc-muted mb-1">
                  {{ t('search.discover') }}
                </div>
                <div class="text-h6 font-weight-bold text-zinc-primary">
                  {{ t('search.discover_title') }}
                </div>
              </div>
              <div v-if="stats" class="stat-pills">
                <span class="stat-pill">
                  <v-icon size="13">mdi-image-multiple</v-icon>
                  {{ stats.photos.toLocaleString() }}
                </span>
                <span class="stat-pill">
                  <v-icon size="13">mdi-video</v-icon>
                  {{ stats.videos.toLocaleString() }}
                </span>
                <span class="stat-pill">
                  <v-icon size="13">mdi-account-group</v-icon>
                  {{ stats.face_photos.toLocaleString() }}
                </span>
              </div>
            </div>

            <!-- Magic toggles -->
            <div class="magic-grid">
              <button
                class="magic-card"
                :class="{ 'magic-card--active': isMediaActive('favoritesOnly') }"
                @click="toggleMedia('favorites')"
              >
                <div class="magic-icon" style="--magic: #f59e0b">
                  <v-icon size="20">mdi-heart</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.favorites') }}</div>
                <div class="magic-count">{{ activeCount('favorites') }}</div>
              </button>
              <button
                class="magic-card"
                :class="{ 'magic-card--active': isMediaActive('videosOnly') }"
                @click="toggleMedia('videos')"
              >
                <div class="magic-icon" style="--magic: #8b5cf6">
                  <v-icon size="20">mdi-video</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.videos') }}</div>
                <div class="magic-count">{{ activeCount('videos') }}</div>
              </button>
              <button
                class="magic-card"
                :class="{ 'magic-card--active': isMediaActive('facesOnly') }"
                @click="toggleMedia('faces')"
              >
                <div class="magic-icon" style="--magic: #0ea5e9">
                  <v-icon size="20">mdi-face-man</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.faces') }}</div>
                <div class="magic-count">{{ activeCount('faces') }}</div>
              </button>
              <button
                class="magic-card"
                :class="{ 'magic-card--active': isMediaActive('papersOnly') }"
                @click="toggleMedia('papers')"
              >
                <div class="magic-icon" style="--magic: #10b981">
                  <v-icon size="20">mdi-file-document-outline</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.papers') }}</div>
                <div class="magic-count">{{ activeCount('papers') }}</div>
              </button>
              <button class="magic-card" @click="surpriseMe">
                <div class="magic-icon" style="--magic: #f43f5e">
                  <v-icon size="20">mdi-dice-multiple</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.surprise') }}</div>
                <div class="magic-count">?</div>
              </button>
            </div>

            <!-- Best shots rail -->
            <div v-if="bestPhotos.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.best_shots') }}</span>
                <span class="section-count">{{ bestPhotos.length }}</span>
              </div>
              <div class="rail">
                <div
                  v-for="photo in bestPhotos"
                  :key="photo.id"
                  class="best-card"
                  @click="searchStore.setAestheticsMin(0.6)"
                >
                  <v-img :src="tileSrc(photo)" cover class="best-img" />
                  <div class="best-badge">
                    <v-icon size="12">mdi-star-four-points</v-icon>
                    {{ Math.round((photo.aesthetics_score ?? 0) * 100) }}
                  </div>
                  <v-icon
                    v-if="photo.favorite"
                    size="14"
                    color="#f59e0b"
                    class="best-fav"
                  >mdi-heart</v-icon>
                </div>
              </div>
            </div>

            <!-- People rail -->
            <div v-if="peopleRow.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.people') }}</span>
                <span class="section-count">{{ peopleRow.length }}</span>
              </div>
              <div class="rail">
                <div
                  v-for="person in peopleRow"
                  :key="person.id"
                  class="face-card"
                  :class="{ 'facet-active': isActive('person', person.id) }"
                  @click="selectPerson(person)"
                >
                  <v-avatar size="56" rounded="xl" class="face-avatar">
                    <v-img v-if="faceSrc(person)" :src="faceSrc(person)" cover />
                    <v-icon v-else>mdi-account</v-icon>
                  </v-avatar>
                  <div class="face-name ellipsis">{{ person.name ?? t('people.unnamed') }}</div>
                  <div class="face-count">{{ person.count }}</div>
                </div>
              </div>
            </div>

            <!-- Places rail -->
            <div v-if="locations.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.locations') }}</span>
                <span class="section-count">{{ locations.length }}</span>
              </div>
              <div class="rail">
                <div
                  v-for="loc in locations"
                  :key="loc.name"
                  class="place-card"
                  :class="{ 'facet-active': isActive('location', loc.name) }"
                  @click="selectLocation(loc.name)"
                >
                  <v-img
                    v-if="locationSrc(loc)"
                    :src="locationSrc(loc)"
                    cover
                    class="place-img"
                  />
                  <div v-else class="place-img place-img--empty">
                    <v-icon size="22" color="rgba(255,255,255,0.7)">mdi-map-marker</v-icon>
                  </div>
                  <div class="place-scrim">
                    <v-icon size="13" class="mr-1">mdi-map-marker</v-icon>
                    <span class="place-name ellipsis">{{ loc.name }}</span>
                    <span class="place-count">{{ loc.count }}</span>
                  </div>
                </div>
              </div>
            </div>

            <!-- Papers -->
            <div v-if="papers.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.papers.title') }}</span>
                <span class="section-count">{{ papers.length }}</span>
              </div>
              <div class="chip-cloud">
                <button
                  v-for="p in papers"
                  :key="p.name"
                  class="cloud-chip"
                  :class="{ 'chip-active': isActive('tag', p.name) }"
                  @click="selectPaper(p.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-file-document-outline</v-icon>
                  {{ labelFromPaper(p.name) }}
                  <span class="cloud-count">{{ p.count }}</span>
                </button>
              </div>
            </div>

            <!-- Cameras -->
            <div v-if="cameras.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.cameras') }}</span>
                <span class="section-count">{{ cameras.length }}</span>
              </div>
              <div class="chip-cloud">
                <button
                  v-for="cam in cameras"
                  :key="cam.name"
                  class="cloud-chip"
                  :class="{ 'chip-active': searchStore.camera === cam.name }"
                  @click="selectCamera(cam.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-camera</v-icon>
                  {{ cam.name }}
                  <span class="cloud-count">{{ cam.count }}</span>
                </button>
              </div>
            </div>

            <!-- Tags cloud -->
            <div v-if="tags.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.tags') }}</span>
                <span class="section-count">{{ tags.length }}</span>
              </div>
              <div class="chip-cloud">
                <button
                  v-for="tag in tags"
                  :key="tag.name"
                  class="cloud-chip"
                  :class="{ 'chip-active': isActive('tag', tag.name) }"
                  @click="selectTag(tag.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-tag-outline</v-icon>
                  {{ tag.name }}
                  <span class="cloud-count">{{ tag.count }}</span>
                </button>
              </div>
            </div>

            <!-- Time -->
            <div v-if="stats && stats.photos" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.dates') }}</span>
              </div>
              <DateRangePicker :model-value="dateRange" @update:model-value="onDateRangeChange" />
            </div>

            <!-- Recent searches -->
            <div v-if="recentSearches.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.recent') }}</span>
              </div>
              <div class="recent-row">
                <button
                  v-for="term in recentSearches"
                  :key="term"
                  class="recent-chip"
                  @click="selectRecent(term)"
                >
                  <v-icon size="14" class="mr-1">mdi-history</v-icon>
                  <span class="ellipsis">{{ term }}</span>
                </button>
              </div>
            </div>

            <div
              v-if="!peopleRow.length && !locations.length && !tags.length && !bestPhotos.length"
              class="pa-3 text-center"
            >
              <div class="text-body-2 text-zinc-muted">{{ t('search.no_data') }}</div>
            </div>
          </template>

          <!-- ============ SEARCH MODE ============ -->
          <template v-else>
            <div class="discover-header">
              <div>
                <div class="text-overline font-weight-black text-zinc-muted mb-1">
                  {{ t('search.results_for', { query: searchStore.query.trim() }) }}
                </div>
              </div>
              <button class="see-all-btn" @click="runSearch">
                {{ t('search.see_all') }}
                <v-icon size="15">mdi-arrow-right</v-icon>
              </button>
            </div>

            <div v-if="inlineLoading" class="pa-4 d-flex justify-center">
              <v-progress-circular indeterminate size="20" width="2" />
            </div>

            <div v-else-if="inlineResults.length" class="inline-grid">
              <div
                v-for="photo in inlineResults"
                :key="photo.id"
                class="inline-thumb"
                @click="runSearch"
              >
                <v-img :src="getMediaThumbnailSrc(photo.location, photo.encoded ?? '', true)" cover />
              </div>
            </div>

            <div v-if="peopleRow.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.people') }}</span>
              </div>
              <div class="rail">
                <div
                  v-for="person in peopleRow"
                  :key="person.id"
                  class="face-card"
                  @click="selectPerson(person)"
                >
                  <v-avatar size="48" rounded="xl" class="face-avatar">
                    <v-img v-if="faceSrc(person)" :src="faceSrc(person)" cover />
                    <v-icon v-else>mdi-account</v-icon>
                  </v-avatar>
                  <div class="face-name ellipsis">{{ person.name }}</div>
                  <div class="face-count">{{ person.count }}</div>
                </div>
              </div>
            </div>

            <div v-if="locations.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.locations') }}</span>
              </div>
              <div class="chip-cloud">
                <button
                  v-for="loc in locations"
                  :key="loc.name"
                  class="cloud-chip"
                  @click="selectLocation(loc.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-map-marker</v-icon>
                  {{ loc.name }}
                  <span class="cloud-count">{{ loc.count }}</span>
                </button>
              </div>
            </div>

            <div v-if="tags.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.tags') }}</span>
              </div>
              <div class="chip-cloud">
                <button
                  v-for="tag in tags"
                  :key="tag.name"
                  class="cloud-chip"
                  @click="selectTag(tag.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-tag-outline</v-icon>
                  {{ tag.name }}
                  <span class="cloud-count">{{ tag.count }}</span>
                </button>
              </div>
            </div>

            <div v-if="cameras.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.cameras') }}</span>
              </div>
              <div class="chip-cloud">
                <button
                  v-for="cam in cameras"
                  :key="cam.name"
                  class="cloud-chip"
                  @click="selectCamera(cam.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-camera</v-icon>
                  {{ cam.name }}
                  <span class="cloud-count">{{ cam.count }}</span>
                </button>
              </div>
            </div>

            <div v-if="stats && stats.photos" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-zinc-muted">{{ t('search.dates') }}</span>
              </div>
              <DateRangePicker :model-value="dateRange" @update:model-value="onDateRangeChange" />
            </div>

            <div
              v-if="!peopleRow.length && !locations.length && !tags.length && !cameras.length && !inlineResults.length && !inlineLoading"
              class="pa-3 text-center"
            >
              <div class="text-body-2 text-zinc-muted">
                {{ t('search.no_matches', { query: searchStore.query.trim() }) }}
              </div>
              <button class="run-search-item mx-auto" @click="runSearch">
                <v-icon size="18" class="mr-2" color="#0ea5e9">mdi-text-search</v-icon>
                {{ t('search.enter_to_search', { query: searchStore.query.trim() }) }}
              </button>
            </div>
          </template>

          <!-- Footer -->
          <div v-if="searchStore.hasFilters" class="active-filters pa-2">
            <span class="text-caption text-zinc-muted mr-2">{{ t('search.active') }}</span>
            <div class="d-flex flex-wrap ga-1">
              <v-chip
                v-for="f in searchStore.activeFilters"
                :key="`${f.type}-${f.value}`"
                size="x-small"
                closable
                variant="tonal"
                @click:close="searchStore.removeFilter(f.type)"
              >
                <v-icon start size="13">{{ iconForFilter(f.type) }}</v-icon>
                {{ f.label || t('people.unnamed') }}
              </v-chip>
            </div>
          </div>

          <v-divider class="border-subtle" />
          <div class="footer-row pa-2">
            <button class="advanced-btn" @click="emit('advanced')">
              <v-icon size="18" class="mr-2">mdi-tune-variant</v-icon>
              {{ t('search.expand') }}
            </button>
            <button v-if="searchStore.hasFilters" class="clear-btn" @click="clearAll">
              {{ t('search.clear_all') }}
            </button>
          </div>
        </template>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.search-wrapper {
  position: relative;
  width: 100%;
}

.search-field {
  display: flex;
  align-items: center;
  gap: 8px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(128, 128, 128, 0.25);
  border-radius: 14px;
  padding: 0 14px;
  height: 44px;
  cursor: text;
}

.search-field:focus-within {
  border-color: rgb(var(--v-theme-primary));
}

.search-icon {
  flex-shrink: 0;
}

.search-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: rgb(var(--v-theme-on-surface));
  font-size: 14px;
}

.search-input::placeholder {
  color: rgba(128, 128, 128, 0.8);
}

.search-dropdown {
  background: var(--color-bg-surface);
  border: 1px solid var(--color-border-default);
  border-radius: 20px;
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.35);
  overflow-y: auto;
  max-height: min(76vh, 640px);
  padding: 10px 0;
  scrollbar-width: thin;
}

.discover-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 18px 10px;
}

.stat-pills {
  display: flex;
  gap: 6px;
}

.stat-pill {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  font-weight: 600;
  color: var(--color-text-secondary);
  background: var(--color-bg-hover);
  border-radius: 999px;
  padding: 4px 10px;
  white-space: nowrap;
}

.magic-grid {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 8px;
  padding: 4px 18px 12px;
}

.magic-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 12px 4px 8px;
  border-radius: 14px;
  border: 1px solid var(--color-border-subtle);
  background: var(--color-bg-hover);
  cursor: pointer;
  user-select: none;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.magic-card:hover {
  transform: translateY(-2px);
  border-color: var(--color-border-hover);
}

.magic-card--active {
  border-color: var(--color-border-hover);
  background: color-mix(in srgb, var(--color-text-primary) 6%, transparent);
}

.magic-icon {
  width: 38px;
  height: 38px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--magic) 16%, transparent);
  color: var(--magic);
}

.magic-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--color-text-primary);
  text-align: center;
  line-height: 1.2;
}

.magic-count {
  font-size: 11px;
  font-weight: 700;
  color: var(--color-text-muted);
}

.discover-section {
  padding: 8px 18px 12px;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.section-count {
  font-size: 11px;
  font-weight: 700;
  color: var(--color-text-muted);
  background: var(--color-bg-hover);
  border-radius: 999px;
  padding: 2px 8px;
}

.rail {
  display: flex;
  gap: 10px;
  overflow-x: auto;
  padding: 2px;
  scrollbar-width: none;
}

.rail::-webkit-scrollbar {
  display: none;
}

.face-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  min-width: 72px;
  padding: 6px 4px;
  border-radius: 14px;
  cursor: pointer;
  user-select: none;
  transition: background 0.15s ease;
}

.face-card:hover {
  background: var(--color-bg-hover);
}

.face-card.facet-active .face-avatar {
  box-shadow: 0 0 0 2px var(--color-text-primary);
}

.face-avatar {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
}

.face-name {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-primary);
  max-width: 72px;
}

.face-count {
  font-size: 11px;
  color: var(--color-text-muted);
}

.best-card {
  position: relative;
  width: 92px;
  height: 122px;
  border-radius: 14px;
  overflow: hidden;
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.best-card:hover {
  transform: translateY(-3px) scale(1.02);
}

.best-img {
  width: 100%;
  height: 100%;
}

.best-badge {
  position: absolute;
  top: 6px;
  left: 6px;
  display: flex;
  align-items: center;
  gap: 2px;
  font-size: 10px;
  font-weight: 700;
  color: #fff;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(6px);
  border-radius: 999px;
  padding: 2px 7px;
}

.best-fav {
  position: absolute;
  top: 7px;
  right: 7px;
  filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.4));
}

.place-card {
  position: relative;
  width: 150px;
  height: 96px;
  border-radius: 14px;
  overflow: hidden;
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.place-card:hover {
  transform: translateY(-3px);
}

.place-card.facet-active {
  box-shadow: 0 0 0 2px var(--color-text-primary);
}

.place-img {
  width: 100%;
  height: 100%;
}

.place-img--empty {
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #3f3f46, #18181b);
}

.place-scrim {
  position: absolute;
  inset: auto 0 0 0;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 10px;
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.72), rgba(0, 0, 0, 0));
}

.place-name {
  flex: 1;
}

.place-count {
  font-size: 10px;
  font-weight: 700;
  background: rgba(255, 255, 255, 0.22);
  border-radius: 999px;
  padding: 1px 6px;
}

.chip-cloud {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.cloud-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary);
  background: var(--color-bg-hover);
  border: 1px solid var(--color-border-subtle);
  border-radius: 999px;
  padding: 5px 12px;
  cursor: pointer;
  user-select: none;
  transition: all 0.15s ease;
}

.cloud-chip:hover {
  border-color: var(--color-border-hover);
  color: var(--color-text-primary);
  transform: translateY(-1px);
}

.cloud-chip.chip-active {
  border-color: var(--color-text-primary);
  color: var(--color-text-primary);
}

.cloud-count {
  font-size: 10px;
  font-weight: 700;
  color: var(--color-text-muted);
  background: color-mix(in srgb, var(--color-text-primary) 8%, transparent);
  border-radius: 999px;
  padding: 1px 6px;
}

.recent-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.recent-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 220px;
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-secondary);
  background: var(--color-bg-hover);
  border-radius: 999px;
  padding: 5px 12px;
  cursor: pointer;
  user-select: none;
}

.recent-chip:hover {
  color: var(--color-text-primary);
}

.inline-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
  gap: 6px;
  padding: 4px 18px 10px;
}

.inline-thumb {
  aspect-ratio: 1;
  border-radius: 10px;
  overflow: hidden;
  cursor: pointer;
}

.inline-thumb :deep(.v-img) {
  width: 100%;
  height: 100%;
}

.see-all-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  font-weight: 700;
  color: rgb(var(--v-theme-primary));
  background: var(--color-bg-hover);
  border-radius: 999px;
  padding: 6px 14px;
  cursor: pointer;
  user-select: none;
}

.see-all-btn:hover {
  filter: brightness(1.05);
}

.run-search-item {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 10px 12px;
  border-radius: 12px;
  cursor: pointer;
  user-select: none;
  font-weight: 600;
  font-size: 13px;
  color: var(--color-text-primary);
}

.run-search-item:hover {
  background: var(--color-bg-hover);
}

.active-filters {
  border-top: 1px solid var(--color-border-subtle);
  margin-top: 6px;
}

.footer-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 4px 10px;
}

.advanced-btn {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  border-radius: 12px;
  cursor: pointer;
  user-select: none;
  font-weight: 600;
  font-size: 13px;
  color: var(--color-text-primary);
}

.advanced-btn:hover {
  background: var(--color-bg-hover);
}

.clear-btn {
  display: inline-flex;
  align-items: center;
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-muted);
  padding: 8px 12px;
  border-radius: 12px;
  cursor: pointer;
  user-select: none;
}

.clear-btn:hover {
  background: var(--color-bg-hover);
  color: var(--color-text-primary);
}

.ellipsis {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 640px) {
  .magic-grid {
    grid-template-columns: repeat(5, 1fr);
    padding: 4px 12px 10px;
  }

  .discover-section {
    padding: 6px 12px 10px;
  }

  .discover-header {
    padding: 4px 12px 8px;
  }
}
</style>
