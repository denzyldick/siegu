<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useSearchStore } from '@/stores/search';
import { useAlbumsStore } from '@/stores/albums';
import { getFaceImageSrc, getMediaThumbnailSrc } from '@/composables/useMediaUtils';
import { listFiles } from '@/services/tauri';
import DateRangePicker from '@/components/search/DateRangePicker.vue';
import BrandIcon from '@/components/search/BrandIcon.vue';
import PersonMatchControls from '@/components/search/PersonMatchControls.vue';
import { brandMeta } from '@/components/search/brands';
import type { FacetGroup, LocationGroup, PhotoTile } from '@/types/search';
import type { MediaItem } from '@/types/media';

const { t } = useI18n();
const searchStore = useSearchStore();
const albumsStore = useAlbumsStore();

const vEdgeScroll = {
  mounted(el: HTMLElement) {
    const update = (): void => {
      el.classList.toggle('edge-scroll', el.scrollWidth > el.clientWidth + 1);
    };
    update();
    if (typeof ResizeObserver !== 'undefined') {
      const ro = new ResizeObserver(update);
      ro.observe(el);
      (el as unknown as { __edgeScrollRO__?: ResizeObserver }).__edgeScrollRO__ = ro;
    }
  },
  unmounted(el: HTMLElement) {
    (el as unknown as { __edgeScrollRO__?: ResizeObserver }).__edgeScrollRO__?.disconnect();
  },
};

const searchWrapRef = ref<HTMLElement | null>(null);
const dropdownOpen = ref(false);
const dropdownStyle = ref<Record<string, string>>({});
const inlineResults = ref<MediaItem[]>([]);
const inlineLoading = ref(false);
let searchTimer: ReturnType<typeof setTimeout> | null = null;
let fetchTimer: ReturnType<typeof setTimeout> | null = null;

const facets = computed(() => searchStore.facets);
const stats = computed(() => facets.value?.stats ?? null);

const q = computed(() => searchStore.query.trim().toLowerCase());

function matches(value: string): boolean {
  return value.toLowerCase().includes(q.value);
}

const namedPeople = computed(() =>
  (facets.value?.people ?? []).filter((p) => !q.value || matches(p.name ?? '')),
);

const unnamedPeople = computed(() =>
  (facets.value?.unnamed_faces ?? []).filter((p) => !q.value || matches(p.name ?? '')),
);

const peopleRow = computed(() => {
  const named = namedPeople.value.slice(0, q.value ? 12 : 14);
  const unnamed = unnamedPeople.value.slice(0, q.value ? 8 : 10);
  return [...named, ...unnamed];
});

const locations = computed(() =>
  (facets.value?.locations ?? []).filter((l) => !q.value || matches(l.name)),
);

const tags = computed(() =>
  (facets.value?.tags ?? []).filter((l) => !q.value || matches(l.name)).slice(0, 8),
);

const papers = computed(() =>
  (facets.value?.papers ?? []).filter((p) => !q.value || matches(p.name)),
);

const cameras = computed(() =>
  (facets.value?.cameras ?? []).filter((c) => !q.value || matches(c.name)),
);

const cameraChips = computed(() =>
  cameras.value
    .map((c) => ({ ...c, hasBrand: brandMeta(c.name) !== null }))
    .filter((c) => c.hasBrand),
);

const dateRange = computed(() => searchStore.dateRange);

const bestPhotos = computed(() => (facets.value?.best_photos ?? []).slice(0, 10));

const recentSearches = computed(() => searchStore.recentSearches);

const isMobile = computed(() => window.innerWidth < 640);

function faceSrc(person: FacetGroup): string {
  return getFaceImageSrc(person.representative_crop, person.encoded);
}

function tileSrc(tile: PhotoTile): string {
  return getMediaThumbnailSrc(tile.location, tile.encoded, true);
}

function locationSrc(group: LocationGroup): string {
  return getMediaThumbnailSrc(group.photo_location ?? '', group.encoded ?? '', true);
}

function repositionDropdown(): void {
  const rect = searchWrapRef.value?.getBoundingClientRect();
  if (!rect) return;
  const viewportH = window.innerHeight;
  const margin = 12;
  const below = viewportH - rect.bottom - margin;
  const above = rect.top - margin;
  const flipUp = below < Math.min(320, above);
  const styles: Record<string, string> = {
    position: 'fixed',
    zIndex: '5000',
  };
  if (isMobile.value) {
    styles.left = '8px';
    styles.right = '8px';
  } else {
    styles.width = `${Math.min(660, Math.max(520, rect.width))}px`;
    styles.left = `${Math.max(8, rect.left + rect.width / 2 - 330)}px`;
  }
  if (flipUp) {
    styles.bottom = `${viewportH - rect.top + margin}px`;
    styles.top = 'auto';
    styles.maxHeight = `${Math.min(720, above)}px`;
  } else {
    styles.top = `${rect.bottom + margin}px`;
    styles.bottom = 'auto';
    styles.maxHeight = `${Math.min(720, below)}px`;
  }
  dropdownStyle.value = styles;
}

function openDropdown(): void {
  dropdownOpen.value = true;
  repositionDropdown();
  if (!searchStore.facets) {
    searchStore.loadFacets();
  }
}

function closeDropdown(): void {
  dropdownOpen.value = false;
}

function selectPerson(person: FacetGroup): void {
  searchStore.togglePerson({ id: person.id, name: person.name ?? '' });
}

function selectLocation(name: string): void {
  searchStore.addFilter({ type: 'location', value: name, label: name });
  closeDropdown();
}

function selectTag(name: string): void {
  searchStore.addFilter({ type: 'tag', value: name, label: name });
  closeDropdown();
}

function selectPaper(name: string): void {
  searchStore.addFilter({ type: 'tag', value: name, label: labelFromPaper(name) });
  closeDropdown();
}

function selectCamera(name: string): void {
  searchStore.setCamera(name);
  closeDropdown();
}

function formatDateLabel(range: [string, string]): string {
  const locale = localStorage.getItem('siegu_language') || 'en';
  const fmt = (d: string) =>
    new Date(`${d}T00:00:00`).toLocaleDateString(locale, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
  return range[0] === range[1] ? fmt(range[0]) : `${fmt(range[0])} — ${fmt(range[1])}`;
}

function onDateRangeChange(range: [string, string] | null): void {
  searchStore.setDateRange(range);
  if (range) {
    searchStore.addFilter({
      type: 'date',
      value: `${range[0]}|${range[1]}`,
      label: formatDateLabel(range),
    });
    if (range[0] !== range[1]) closeDropdown();
  } else {
    searchStore.removeFilter('date');
  }
}

function toggleMedia(type: 'favorites' | 'videos' | 'faces' | 'papers' | 'nsfw'): void {
  if (type === 'favorites') searchStore.toggleFavoriteOnly();
  if (type === 'videos') searchStore.toggleVideoOnly();
  if (type === 'faces') searchStore.toggleFacesOnly();
  if (type === 'papers') searchStore.togglePapersOnly();
  if (type === 'nsfw') searchStore.toggleNsfwOnly();
  closeDropdown();
}

function surpriseMe(): void {
  searchStore.clearFilters();
  searchStore.toggleSurprise();
  closeDropdown();
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
  };
  return map[name] ?? name;
}

function runSearch(): void {
  const term = searchStore.query.trim();
  if (!term) return;
  searchStore.addRecentSearch(term);
  closeDropdown();
}

function selectRecent(term: string): void {
  searchStore.setQuery(term);
  closeDropdown();
}

function clearAll(): void {
  searchStore.clearQuery();
  searchStore.clearFilters();
  closeDropdown();
}

const saveAlbumDialog = ref(false);
const saveAlbumName = ref('');
const savingAlbum = ref(false);

function canSaveAlbum(): boolean {
  return searchStore.hasFilters || searchStore.hasQuery;
}

const isEditingAlbum = computed(() => albumsStore.editingSmartAlbum !== null);

function buildRule(): Record<string, unknown> {
  const byType = (type: string) => searchStore.activeFilters.find((f) => f.type === type);
  const people = searchStore.activeFilters.filter((f) => f.type === 'person');
  const location = byType('location');
  const tag = byType('tag');
  const month = byType('month');
  const date = byType('date');
  const dateRange = date ? (date.value.split('|') as [string, string]) : null;
  return {
    person_ids: people.map((f) => f.value),
    person_match: searchStore.personMatch,
    person_alone: searchStore.personAlone,
    location: location ? location.value : null,
    tag: tag ? tag.value : null,
    date_from: month ? `${month.value}-01` : dateRange ? dateRange[0] : null,
    date_to: month ? `${month.value}-31` : dateRange ? dateRange[1] : null,
    query: searchStore.query.trim() || null,
    videos: searchStore.mediaFilters.videosOnly || null,
    favorite: searchStore.mediaFilters.favoritesOnly,
    has_faces: searchStore.mediaFilters.facesOnly,
    papers: searchStore.mediaFilters.papersOnly,
    nsfw_only: searchStore.mediaFilters.nsfwOnly,
    camera: searchStore.camera,
    aesthetics_min: searchStore.aestheticsMin,
    random: false,
    order_by: searchStore.sortOrder,
    album_id: null,
  };
}

function openSaveAlbumDialog(): void {
  if (!canSaveAlbum()) return;
  saveAlbumName.value = albumsStore.editingSmartAlbum?.name ?? '';
  saveAlbumDialog.value = true;
}

async function saveAsAlbum(): Promise<void> {
  const name = saveAlbumName.value.trim();
  if (!name || savingAlbum.value) return;
  savingAlbum.value = true;
  try {
    const editing = albumsStore.editingSmartAlbum;
    if (editing) {
      await albumsStore.updateSmartAlbumRule(editing.id, buildRule());
    } else {
      await albumsStore.createSmartAlbum(name, buildRule(), 'smart');
    }
    albumsStore.stopEditingSmartAlbum();
    saveAlbumDialog.value = false;
    closeDropdown();
  } finally {
    savingAlbum.value = false;
  }
}

function isActive(type: string, value: string): boolean {
  return searchStore.activeFilters.some((f) => f.type === type && f.value === value);
}

function isMediaActive(
  key: 'favoritesOnly' | 'videosOnly' | 'facesOnly' | 'papersOnly' | 'nsfwOnly',
): boolean {
  return searchStore.mediaFilters[key];
}

function activeCount(type: string): number {
  const counts: Record<string, string | number> = {
    favorites: stats.value?.favorites ?? 0,
    videos: stats.value?.videos ?? 0,
    faces: stats.value?.face_photos ?? 0,
    papers: (facets.value?.papers ?? []).reduce((sum, p) => sum + p.count, 0),
    nsfw: stats.value?.nsfw_photos ?? 0,
  };
  return counts[type] as number;
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter') {
    runSearch();
  } else if (event.key === 'Escape') {
    closeDropdown();
  }
}

function onDocumentClick(event: MouseEvent): void {
  if (!dropdownOpen.value) return;
  const target = event.target as Node;
  if (searchWrapRef.value && !searchWrapRef.value.contains(target)) {
    closeDropdown();
  }
}

function onScroll(): void {
  if (dropdownOpen.value) {
    repositionDropdown();
  }
}

watch(q, () => {
  if (!dropdownOpen.value) return;
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    if (fetchTimer) clearTimeout(fetchTimer);
    fetchTimer = setTimeout(async () => {
      const term = searchStore.query.trim();
      if (!term) {
        inlineResults.value = [];
        return;
      }
      inlineLoading.value = true;
      try {
        const results = await listFiles({ offset: 0, limit: 12, query: term });
        inlineResults.value = results;
      } catch (e) {
        console.error('[SearchBar] inline results failed:', e);
      } finally {
        inlineLoading.value = false;
      }
    }, 180);
  }, 120);
});

onMounted(() => {
  searchStore.loadFacets();
  document.addEventListener('click', onDocumentClick);
  window.addEventListener('resize', onScroll);
  window.addEventListener('scroll', onScroll, true);
});

onUnmounted(() => {
  document.removeEventListener('click', onDocumentClick);
  window.removeEventListener('resize', onScroll);
  window.removeEventListener('scroll', onScroll, true);
  if (searchTimer) clearTimeout(searchTimer);
  if (fetchTimer) clearTimeout(fetchTimer);
});

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
  );
}
</script>

<template>
  <div ref="searchWrapRef" class="search-wrapper">
    <div class="search-field" data-tour="search" @click="openDropdown">
      <v-icon size="20" class="search-icon" color="rgba(var(--v-theme-on-surface), 0.7)"
        >mdi-magnify</v-icon
      >
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
        color="rgba(var(--v-theme-on-surface), 0.7)"
        class="cursor-pointer"
        @click.stop="clearAll"
      >
        mdi-close-circle
      </v-icon>
    </div>

    <Teleport to="body">
      <div v-if="dropdownOpen" class="search-dropdown" :style="dropdownStyle" @click.stop>
        <div v-if="!facets" class="pa-6 d-flex justify-center">
          <v-progress-circular indeterminate size="24" width="2" />
        </div>

        <template v-else>
          <!-- ============ BROWSE MODE ============ -->
          <template v-if="!q">
            <div class="discover-header">
              <div>
                <div class="text-overline font-weight-black text-disabled mb-1">
                  {{ t('search.discover') }}
                </div>
                <div class="text-h6 font-weight-bold text-high-emphasis">
                  {{ t('search.discover_title') }}
                </div>
              </div>
            </div>

            <!-- Magic toggles -->
            <div v-edge-scroll class="magic-grid">
              <v-btn
                class="magic-card"
                :class="{ 'magic-card--active': isMediaActive('favoritesOnly') }"
                @click="toggleMedia('favorites')"
              >
                <div class="magic-icon" style="--magic: var(--color-brand-favorite)">
                  <v-icon size="20">mdi-heart</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.favorites') }}</div>
                <div class="magic-count">{{ activeCount('favorites') }}</div>
              </v-btn>
              <v-btn
                class="magic-card"
                :class="{ 'magic-card--active': isMediaActive('videosOnly') }"
                @click="toggleMedia('videos')"
              >
                <div class="magic-icon" style="--magic: var(--color-brand-videos)">
                  <v-icon size="20">mdi-video</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.videos') }}</div>
                <div class="magic-count">{{ activeCount('videos') }}</div>
              </v-btn>
              <v-btn
                class="magic-card"
                :class="{ 'magic-card--active': isMediaActive('facesOnly') }"
                @click="toggleMedia('faces')"
              >
                <div class="magic-icon" style="--magic: var(--color-brand-faces)">
                  <v-icon size="20">mdi-face-man</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.faces') }}</div>
                <div class="magic-count">{{ activeCount('faces') }}</div>
              </v-btn>
              <v-btn
                class="magic-card"
                :class="{ 'magic-card--active': isMediaActive('papersOnly') }"
                @click="toggleMedia('papers')"
              >
                <div class="magic-icon" style="--magic: var(--color-brand-papers)">
                  <v-icon size="20">mdi-file-document-outline</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.papers') }}</div>
                <div class="magic-count">{{ activeCount('papers') }}</div>
              </v-btn>
              <v-btn
                v-if="activeCount('nsfw') > 0"
                class="magic-card"
                :class="{ 'magic-card--active': isMediaActive('nsfwOnly') }"
                @click="toggleMedia('nsfw')"
              >
                <div class="magic-icon" style="--magic: var(--color-brand-nsfw)">
                  <v-icon size="20">mdi-alert-octagon</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.nsfw') }}</div>
                <div class="magic-count">{{ activeCount('nsfw') }}</div>
              </v-btn>
              <v-btn class="magic-card" @click="surpriseMe">
                <div class="magic-icon" style="--magic: var(--color-brand-surprise)">
                  <v-icon size="20">mdi-dice-multiple</v-icon>
                </div>
                <div class="magic-label">{{ t('search.magic.surprise') }}</div>
                <div class="magic-count">?</div>
              </v-btn>
            </div>

            <!-- Recent searches -->
            <div v-if="recentSearches.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.recent') }}</span>
              </div>
              <div class="recent-row">
                <v-btn
                  v-for="term in recentSearches"
                  :key="term"
                  class="recent-chip"
                  @click="selectRecent(term)"
                >
                  <v-icon size="14" class="mr-1">mdi-history</v-icon>
                  <span class="ellipsis">{{ term }}</span>
                </v-btn>
              </div>
            </div>

            <!-- Best shots rail -->
            <div v-if="bestPhotos.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.best_shots') }}</span>
                <span class="section-count">{{ bestPhotos.length }}</span>
              </div>
              <div v-edge-scroll class="rail">
                <div
                  v-for="photo in bestPhotos"
                  :key="photo.id"
                  class="best-card"
                  @click="searchStore.setAestheticsMin(0.6)"
                >
                  <v-img :src="tileSrc(photo)" cover class="best-img" />
                  <div class="best-badge" :title="t('search.best_shot')">
                    <v-icon size="12">mdi-star-four-points</v-icon>
                  </div>
                  <v-icon
                    v-if="photo.favorite"
                    size="14"
                    color="var(--color-brand-favorite)"
                    class="best-fav"
                    >mdi-heart</v-icon
                  >
                </div>
              </div>
            </div>

            <!-- People rail -->
            <div v-if="peopleRow.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.people') }}</span>
                <span class="section-count">{{ peopleRow.length }}</span>
              </div>
              <div v-edge-scroll class="rail">
                <div
                  v-for="person in peopleRow"
                  :key="person.id"
                  class="face-card"
                  :class="{
                    'facet-active': isActive('person', person.id),
                    'face-card--unnamed': !person.name,
                  }"
                  @click="selectPerson(person)"
                >
                  <div class="face-avatar-wrap">
                    <v-avatar size="56" rounded="xl" class="face-avatar">
                      <v-img v-if="faceSrc(person)" :src="faceSrc(person)" cover />
                      <v-icon v-else>mdi-account</v-icon>
                    </v-avatar>
                    <div v-if="!person.name" class="face-unnamed-badge">
                      <v-icon size="10">mdi-help</v-icon>
                    </div>
                  </div>
                  <div class="face-name ellipsis">{{ person.name }}</div>
                  <div class="face-count">{{ person.count }}</div>
                </div>
              </div>
            </div>

            <!-- Places rail -->
            <div v-if="locations.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.locations') }}</span>
                <span class="section-count">{{ locations.length }}</span>
              </div>
              <div v-edge-scroll class="rail">
                <div
                  v-for="loc in locations"
                  :key="loc.name"
                  class="place-card"
                  :class="{ 'facet-active': isActive('location', loc.name) }"
                  @click="selectLocation(loc.name)"
                >
                  <v-img v-if="locationSrc(loc)" :src="locationSrc(loc)" cover class="place-img" />
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
                <span class="text-overline text-disabled">{{ t('search.papers.title') }}</span>
                <span class="section-count">{{ papers.length }}</span>
              </div>
              <div v-edge-scroll class="chip-cloud">
                <v-btn
                  v-for="p in papers"
                  :key="p.name"
                  class="cloud-chip"
                  :class="{ 'chip-active': isActive('tag', p.name) }"
                  @click="selectPaper(p.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-file-document-outline</v-icon>
                  {{ labelFromPaper(p.name) }}
                  <span class="cloud-count">{{ p.count }}</span>
                </v-btn>
              </div>
            </div>

            <!-- Tags cloud -->
            <div v-if="tags.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.tags') }}</span>
                <span class="section-count">{{ tags.length }}</span>
              </div>
              <div v-edge-scroll class="chip-cloud">
                <v-btn
                  v-for="tag in tags"
                  :key="tag.name"
                  class="cloud-chip"
                  :class="{ 'chip-active': isActive('tag', tag.name) }"
                  @click="selectTag(tag.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-tag-outline</v-icon>
                  {{ tag.name }}
                  <span class="cloud-count">{{ tag.count }}</span>
                </v-btn>
              </div>
            </div>

            <!-- Time -->
            <div class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.dates') }}</span>
              </div>
              <DateRangePicker :model-value="dateRange" @update:model-value="onDateRangeChange" />
            </div>

            <!-- Cameras -->
            <div v-if="cameraChips.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.cameras') }}</span>
                <span class="section-count">{{ cameraChips.length }}</span>
              </div>
              <div v-edge-scroll class="chip-cloud">
                <v-btn
                  v-for="cam in cameraChips"
                  :key="cam.name"
                  class="cloud-chip cloud-chip--brand"
                  :class="{ 'chip-active': searchStore.camera === cam.name }"
                  :title="cam.name"
                  @click="selectCamera(cam.name)"
                >
                  <BrandIcon :name="cam.name" :size="20" />
                </v-btn>
              </div>
            </div>

            <div
              v-if="!peopleRow.length && !locations.length && !tags.length && !bestPhotos.length"
              class="pa-3 text-center"
            >
              <div class="text-body-2 text-disabled">{{ t('search.no_data') }}</div>
            </div>
          </template>

          <!-- ============ SEARCH MODE ============ -->
          <template v-else>
            <div class="discover-header">
              <div>
                <div class="text-overline font-weight-black text-disabled mb-1">
                  {{ t('search.results_for', { query: searchStore.query.trim() }) }}
                </div>
              </div>
              <v-btn class="see-all-btn" @click="runSearch">
                {{ t('search.see_all') }}
                <v-icon size="15">mdi-arrow-right</v-icon>
              </v-btn>
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
                <v-img
                  :src="getMediaThumbnailSrc(photo.location, photo.encoded ?? '', true)"
                  cover
                />
              </div>
            </div>

            <div v-if="peopleRow.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.people') }}</span>
              </div>
              <div v-edge-scroll class="rail">
                <div
                  v-for="person in peopleRow"
                  :key="person.id"
                  class="face-card"
                  :class="{
                    'facet-active': isActive('person', person.id),
                    'face-card--unnamed': !person.name,
                  }"
                  @click="selectPerson(person)"
                >
                  <div class="face-avatar-wrap">
                    <v-avatar size="48" rounded="xl" class="face-avatar">
                      <v-img v-if="faceSrc(person)" :src="faceSrc(person)" cover />
                      <v-icon v-else>mdi-account</v-icon>
                    </v-avatar>
                    <div v-if="!person.name" class="face-unnamed-badge">
                      <v-icon size="10">mdi-help</v-icon>
                    </div>
                  </div>
                  <div class="face-name ellipsis">{{ person.name }}</div>
                  <div class="face-count">{{ person.count }}</div>
                </div>
              </div>
            </div>

            <div v-if="locations.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.locations') }}</span>
              </div>
              <div v-edge-scroll class="chip-cloud">
                <v-btn
                  v-for="loc in locations"
                  :key="loc.name"
                  class="cloud-chip"
                  @click="selectLocation(loc.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-map-marker</v-icon>
                  {{ loc.name }}
                  <span class="cloud-count">{{ loc.count }}</span>
                </v-btn>
              </div>
            </div>

            <div v-if="tags.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.tags') }}</span>
              </div>
              <div v-edge-scroll class="chip-cloud">
                <v-btn
                  v-for="tag in tags"
                  :key="tag.name"
                  class="cloud-chip"
                  @click="selectTag(tag.name)"
                >
                  <v-icon size="14" class="mr-1">mdi-tag-outline</v-icon>
                  {{ tag.name }}
                  <span class="cloud-count">{{ tag.count }}</span>
                </v-btn>
              </div>
            </div>

            <div v-if="cameras.length" class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.cameras') }}</span>
              </div>
              <div v-edge-scroll class="chip-cloud">
                <v-btn
                  v-for="cam in cameraChips"
                  :key="cam.name"
                  class="cloud-chip cloud-chip--brand"
                  :class="{ 'chip-active': searchStore.camera === cam.name }"
                  :title="cam.name"
                  @click="selectCamera(cam.name)"
                >
                  <BrandIcon :name="cam.name" :size="20" />
                </v-btn>
              </div>
            </div>

            <div class="discover-section">
              <div class="section-header">
                <span class="text-overline text-disabled">{{ t('search.dates') }}</span>
              </div>
              <DateRangePicker :model-value="dateRange" @update:model-value="onDateRangeChange" />
            </div>

            <div
              v-if="
                !peopleRow.length &&
                !locations.length &&
                !tags.length &&
                !cameras.length &&
                !inlineResults.length &&
                !inlineLoading
              "
              class="pa-3 text-center"
            >
              <div class="text-body-2 text-disabled">
                {{ t('search.no_matches', { query: searchStore.query.trim() }) }}
              </div>
              <v-btn class="run-search-item mx-auto" @click="runSearch">
                <v-icon size="18" class="mr-2" color="var(--color-brand-faces)"
                  >mdi-text-search</v-icon
                >
                {{ t('search.enter_to_search', { query: searchStore.query.trim() }) }}
              </v-btn>
            </div>
          </template>

          <!-- Footer -->
          <div v-if="searchStore.hasFilters" class="active-filters pa-2">
            <span class="text-caption text-disabled mr-2">{{ t('search.active') }}</span>
            <div class="d-flex flex-wrap ga-1">
              <v-chip
                v-for="f in searchStore.activeFilters"
                :key="`${f.type}-${f.value}`"
                size="x-small"
                closable
                variant="tonal"
                @click:close="searchStore.removeFilterValue(f.type, f.value)"
              >
                <v-icon start size="13">{{ iconForFilter(f.type) }}</v-icon>
                {{ f.label }}
              </v-chip>
            </div>
            <PersonMatchControls v-if="searchStore.personCount > 0" class="mt-1" />
          </div>

          <v-divider class="border" />
          <div v-if="searchStore.hasFilters || searchStore.hasQuery" class="footer-row pa-2">
            <v-btn class="save-album-btn" :disabled="!canSaveAlbum()" @click="openSaveAlbumDialog">
              <v-icon size="14" class="mr-1">mdi-content-save-outline</v-icon>
              {{ isEditingAlbum ? t('search.update_album') : t('search.save_as_album') }}
            </v-btn>
            <v-btn class="clear-btn" @click="clearAll">
              {{ t('search.clear_all') }}
            </v-btn>
          </div>
        </template>
      </div>
    </Teleport>

    <v-dialog v-model="saveAlbumDialog" max-width="420">
      <v-card class="rounded-xl pa-6" color="surface">
        <h3 class="text-h6 font-weight-bold text-high-emphasis mb-1">
          {{ isEditingAlbum ? t('search.update_album') : t('search.save_as_album') }}
        </h3>
        <p class="text-caption text-disabled mb-4">
          {{ isEditingAlbum ? t('search.update_album_hint') : t('search.save_as_album_hint') }}
        </p>
        <v-text-field
          v-if="!isEditingAlbum"
          v-model="saveAlbumName"
          :label="t('search.save_as_album_placeholder')"
          variant="outlined"
          hide-details
          @keyup.enter="saveAsAlbum"
        ></v-text-field>
        <div class="d-flex justify-end mt-4 ga-2">
          <v-btn variant="text" @click="saveAlbumDialog = false">{{ t('common.cancel') }}</v-btn>
          <v-btn
            variant="flat"
            color="primary"
            class="px-6"
            :disabled="!isEditingAlbum && !saveAlbumName.trim()"
            :loading="savingAlbum"
            @click="saveAsAlbum"
          >
            {{ isEditingAlbum ? t('common.update') : t('common.save') }}
          </v-btn>
        </div>
      </v-card>
    </v-dialog>
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
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  border-radius: var(--radius-lg);
  padding: 0 14px;
  height: 44px;
  cursor: text;
}

.search-field:focus-within {
  border-color: rgb(var(--v-theme-on-surface));
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
  color: rgba(var(--v-theme-on-surface), 0.6);
}

.search-dropdown {
  background: rgb(var(--v-theme-surface));
  border-radius: var(--radius-2xl);
  box-shadow: var(--shadow-popover);
  overflow-y: auto;
  max-height: min(76vh, 640px);
  padding: 10px 0;
  scrollbar-width: thin;
}

.search-dropdown button {
  border: none;
  -webkit-appearance: none;
  appearance: none;
}

.discover-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 18px 10px;
}

.magic-grid {
  display: flex;
  flex-wrap: nowrap;
  align-items: stretch;
  gap: 10px;
  padding: 8px 18px 14px;
  overflow-x: auto;
  scrollbar-width: none;
}

.magic-grid::-webkit-scrollbar {
  display: none;
}

.magic-card {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
  width: 88px;
  height: 88px;
  padding: 12px;
  border-radius: var(--radius-lg);
  border: none;
  -webkit-appearance: none;
  appearance: none;
  background: rgb(var(--v-theme-surface-light));
  cursor: pointer;
  user-select: none;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.magic-card:hover {
  transform: translateY(-2px);
  background: color-mix(in srgb, rgb(var(--v-theme-on-surface)) 6%, transparent);
}

.magic-card--active {
  background: color-mix(in srgb, rgb(var(--v-theme-on-surface)) 8%, transparent);
}

.magic-icon {
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--magic) 16%, transparent);
  color: var(--magic);
}

.magic-label {
  font-size: 11px;
  font-weight: 600;
  color: rgb(var(--v-theme-on-surface));
  text-align: center;
  line-height: 1.2;
}

.magic-count {
  font-size: 11px;
  font-weight: 700;
  color: rgba(var(--v-theme-on-surface), 0.6);
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
  color: rgba(var(--v-theme-on-surface), 0.6);
  background: rgb(var(--v-theme-surface-light));
  border-radius: var(--radius-pill);
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
  border-radius: var(--radius-lg);
  cursor: pointer;
  user-select: none;
  transition: background 0.15s ease;
}

.face-card:hover {
  background: rgb(var(--v-theme-surface-light));
}

.face-card.facet-active .face-avatar {
  box-shadow: 0 0 0 2px rgb(var(--v-theme-on-surface));
}

.face-avatar {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
}

.face-avatar-wrap {
  position: relative;
}

.face-card--unnamed .face-avatar {
  box-shadow: 0 0 0 2px color-mix(in srgb, rgba(var(--v-theme-on-surface), 0.6) 45%, transparent);
}

.face-unnamed-badge {
  position: absolute;
  top: -3px;
  right: -3px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: rgba(var(--v-theme-on-surface), 0.6);
  color: rgb(var(--v-theme-surface));
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px solid rgb(var(--v-theme-surface));
}

.face-name {
  font-size: 12px;
  font-weight: 600;
  color: rgb(var(--v-theme-on-surface));
  max-width: 72px;
}

.face-count {
  font-size: 11px;
  color: rgba(var(--v-theme-on-surface), 0.6);
}

.best-card {
  position: relative;
  width: 92px;
  height: 122px;
  border-radius: var(--radius-lg);
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
  border-radius: var(--radius-pill);
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
  border-radius: var(--radius-lg);
  overflow: hidden;
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.place-card:hover {
  transform: translateY(-3px);
}

.place-card.facet-active {
  box-shadow: 0 0 0 2px rgb(var(--v-theme-on-surface));
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
  border-radius: var(--radius-pill);
  padding: 1px 6px;
}

.chip-cloud {
  display: flex;
  flex-wrap: nowrap;
  gap: 6px;
  overflow-x: auto;
  padding: 2px;
  scrollbar-width: none;
}

.chip-cloud::-webkit-scrollbar {
  display: none;
}

.edge-scroll {
  -webkit-mask-image: linear-gradient(to right, black calc(100% - 24px), transparent);
  mask-image: linear-gradient(to right, black calc(100% - 24px), transparent);
}

.cloud-chip {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
  font-size: 12px;
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.7);
  background: rgb(var(--v-theme-surface-light));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  border-radius: var(--radius-pill);
  padding: 5px 12px;
  cursor: pointer;
  user-select: none;
  transition: all 0.15s ease;
}

.cloud-chip:hover {
  border-color: rgba(var(--v-theme-on-surface), 0.4);
  color: rgb(var(--v-theme-on-surface));
  transform: translateY(-1px);
}

.cloud-chip.chip-active {
  border-color: rgb(var(--v-theme-on-surface));
  color: rgb(var(--v-theme-on-surface));
}

.cloud-chip--brand {
  padding: 8px 14px;
  justify-content: center;
}

.cloud-chip--brand .brand-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.cloud-count {
  font-size: 10px;
  font-weight: 700;
  white-space: nowrap;
  color: rgba(var(--v-theme-on-surface), 0.6);
  background: color-mix(in srgb, rgb(var(--v-theme-on-surface)) 8%, transparent);
  border-radius: var(--radius-pill);
  padding: 1px 6px;
}

.recent-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.recent-chip {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 220px;
  font-size: 12px;
  font-weight: 500;
  color: rgba(var(--v-theme-on-surface), 0.7);
  background: rgb(var(--v-theme-surface-light));
  border-radius: var(--radius-pill);
  padding: 5px 12px;
  cursor: pointer;
  user-select: none;
}

.recent-chip:hover {
  color: rgb(var(--v-theme-on-surface));
}

.inline-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
  gap: 6px;
  padding: 4px 18px 10px;
}

.inline-thumb {
  aspect-ratio: 1;
  border-radius: var(--radius-md);
  overflow: hidden;
  cursor: pointer;
}

.inline-thumb :deep(.v-img) {
  width: 100%;
  height: 100%;
}

.see-all-btn {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  font-weight: 700;
  color: rgb(var(--v-theme-primary));
  background: rgb(var(--v-theme-surface-light));
  border-radius: var(--radius-pill);
  padding: 6px 14px;
  cursor: pointer;
  user-select: none;
}

.see-all-btn:hover {
  filter: brightness(1.05);
}

.run-search-item {
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 10px 12px;
  border-radius: var(--radius-md);
  cursor: pointer;
  user-select: none;
  font-weight: 600;
  font-size: 13px;
  color: rgb(var(--v-theme-on-surface));
}

.run-search-item:hover {
  background: rgb(var(--v-theme-surface-light));
}

.active-filters {
  border-top: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  margin-top: 6px;
}

.footer-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 4px 10px;
}

.clear-btn {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  font-size: 12px;
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.6);
  padding: 8px 12px;
  border-radius: var(--radius-md);
  cursor: pointer;
  user-select: none;
}

.clear-btn:hover {
  background: rgb(var(--v-theme-surface-light));
  color: rgb(var(--v-theme-on-surface));
}

.save-album-btn {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  font-size: 12px;
  font-weight: 600;
  color: rgb(var(--v-theme-primary));
  padding: 8px 12px;
  border-radius: var(--radius-md);
  cursor: pointer;
  user-select: none;
}

.save-album-btn:hover:not(:disabled) {
  background: rgb(var(--v-theme-surface-light));
}

.save-album-btn:disabled {
  opacity: 0.4;
  cursor: default;
}

.ellipsis {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 640px) {
  .magic-grid {
    grid-template-columns: repeat(5, 1fr);
    padding: 8px 12px 12px;
  }

  .discover-section {
    padding: 6px 12px 10px;
  }

  .discover-header {
    padding: 4px 12px 8px;
  }
}
</style>
