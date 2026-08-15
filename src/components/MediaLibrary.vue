<template>
  <div class="media-library-container px-4 py-6">
    <v-fade-transition>
      <div v-if="selectedIds.size > 0" class="bulk-toolbar-container">
        <v-sheet
          class="bulk-toolbar d-flex align-center px-6 py-3 rounded-pill elevation-3"
          color="rgb(var(--v-theme-primary))"
        >
          <v-btn
            icon="mdi-close"
            variant="text"
            density="comfortable"
            color="rgb(var(--v-theme-on-primary))"
            @click="clearSelection"
          ></v-btn>
          <div class="ml-4">
            <div
              class="text-subtitle-2 font-weight-bold"
              style="color: rgb(var(--v-theme-on-primary))"
            >
              {{ $t('media.items_selected', { count: selectedIds.size }) }}
            </div>
          </div>
          <v-spacer></v-spacer>
          <div class="d-flex ga-2">
            <v-btn variant="flat" class="px-6" size="small" @click="bulkFavorite">
              <v-icon size="16" class="mr-2">mdi-heart</v-icon>
              <span>{{ $t('media.favorite') }}</span>
            </v-btn>
            <v-btn
              variant="flat"
              color="rgba(var(--v-theme-on-primary), 0.1)"
              class="px-6 rounded-xl text-none font-weight-bold"
              style="color: rgb(var(--v-theme-on-primary))"
              size="small"
              @click="addToAlbumOpen = true"
            >
              <v-icon size="16" class="mr-2">mdi-image-plus</v-icon>
              <span>{{ $t('media.add_to_album') }}</span>
            </v-btn>
            <v-btn
              variant="flat"
              color="rgba(var(--v-theme-on-primary), 0.1)"
              class="px-6 rounded-xl text-none font-weight-bold"
              style="color: rgb(var(--v-theme-on-primary))"
              size="small"
              @click="bulkRemove"
            >
              {{ $t('media.remove') }}
            </v-btn>
          </div>
        </v-sheet>
      </div>
    </v-fade-transition>

    <div v-if="showTrips" class="px-2 mb-6 animate-fade-in">
      <div class="d-flex align-center px-2 mb-3">
        <h2 class="section-title text-subtitle-1 font-weight-bold text-high-emphasis flex-grow-1">
          {{ $t('media.trips_title') }}
        </h2>
        <v-btn size="small" variant="text" class="ml-1" @click="goToTrips">
          <v-icon start size="13">mdi-airplane</v-icon>
          {{ $t('media.trips_view_all') }}
        </v-btn>
      </div>
      <div class="trips-scroll">
        <div v-for="trip in trips" :key="trip.id" class="trip-card" @click="openTrip(trip)">
          <div class="trip-cover">
            <img
              v-if="tripCoverSrc(trip)"
              :src="tripCoverSrc(trip)"
              :alt="trip.name"
              loading="lazy"
              class="trip-cover-img"
            />
            <div v-else class="trip-cover-placeholder d-flex align-center justify-center">
              <v-icon size="40" color="rgba(var(--v-theme-on-surface), 0.25)">mdi-airplane</v-icon>
            </div>
            <div class="trip-count">
              <v-icon size="12">mdi-image</v-icon>
              {{ $t('media.items_count', { count: trip.count }) }}
            </div>
          </div>
          <div class="trip-name text-subtitle-2 font-weight-bold text-high-emphasis">
            {{ trip.name }}
          </div>
        </div>
      </div>
    </div>

    <DynamicScroller
      v-if="groups.length > 0 && useVirtualScroller"
      class="animate-fade-in"
      :items="virtualItems"
      :min-item-size="280"
      key-field="key"
      page-mode
      v-slot="{ item, active }"
    >
      <DynamicScrollerItem :item="item" :active="active">
        <div v-if="item.type === 'header'" class="month-header mb-3">
          <div class="d-flex align-center px-2 py-3 rounded-lg header-blur">
            <h2 class="text-h5 font-weight-bold text-high-emphasis letter-spacing-tight">
              {{ item.name }}
            </h2>
            <v-spacer></v-spacer>
            <span
              class="text-caption text-disabled font-weight-medium px-3 py-1 rounded-pill border"
            >
              {{ $t('media.items_count', { count: item.count }) }}
            </span>
          </div>
        </div>
        <div v-else class="photo-row" :style="{ gridTemplateColumns: `repeat(${columns}, 1fr)` }">
          <MediaCard
            v-for="photo in item.photos"
            :key="photo.id"
            :path="photo"
            :selected="selectedIds.has(photo.id)"
            :selection-mode="selectedIds.size > 0"
            @click="openViewerByPhoto(photo)"
            @select="toggleSelection"
            @toggle-favorite="handleToggleFavorite"
            @not-synced="handleNotSynced"
          />
        </div>
      </DynamicScrollerItem>
    </DynamicScroller>

    <div v-else-if="groups.length > 0" class="animate-fade-in">
      <div v-for="item in virtualItems" :key="item.key">
        <div v-if="item.type === 'header'" class="month-header mb-3">
          <div class="d-flex align-center px-2 py-3 rounded-lg header-blur">
            <h2 class="text-h5 font-weight-bold text-high-emphasis letter-spacing-tight">
              {{ item.name }}
            </h2>
            <v-spacer></v-spacer>
            <span
              class="text-caption text-disabled font-weight-medium px-3 py-1 rounded-pill border"
            >
              {{ $t('media.items_count', { count: item.count }) }}
            </span>
          </div>
        </div>
        <div v-else class="photo-row" :style="{ gridTemplateColumns: `repeat(${columns}, 1fr)` }">
          <MediaCard
            v-for="photo in item.photos"
            :key="photo.id"
            :path="photo"
            :selected="selectedIds.has(photo.id)"
            :selection-mode="selectedIds.size > 0"
            @click="openViewerByPhoto(photo)"
            @select="toggleSelection"
            @toggle-favorite="handleToggleFavorite"
            @not-synced="handleNotSynced"
          />
        </div>
      </div>
    </div>

    <div
      v-else-if="!loading && isScanning"
      class="empty-state-container d-flex flex-column align-center justify-center text-center"
    >
      <div class="empty-state-icon mb-6">
        <v-progress-circular
          size="64"
          width="4"
          indeterminate
          color="rgba(var(--v-theme-on-surface), 0.45)"
        ></v-progress-circular>
      </div>

      <h3 class="text-h5 font-weight-bold text-high-emphasis mb-2">
        {{ $t('media.scanning_in_progress') }}
      </h3>
      <p class="text-body-1 text-medium-emphasis max-w-400 mx-auto">
        {{ $t('media.scanning_hint') }}
      </p>
    </div>

    <div
      v-else-if="!loading"
      class="empty-state-container d-flex flex-column align-center justify-center text-center"
    >
      <div class="empty-state-icon mb-6">
        <template v-if="searchQuery">
          <v-icon size="80" color="rgba(var(--v-theme-on-surface), 0.25)"
            >mdi-text-search-variant</v-icon
          >
        </template>
        <template v-else-if="filters.favoritesOnly">
          <v-icon size="80" color="rgba(var(--v-theme-error), 0.12)">mdi-heart-multiple</v-icon>
        </template>
        <template v-else>
          <v-icon size="80" color="rgba(var(--v-theme-on-surface), 0.25)"
            >mdi-image-multiple-outline</v-icon
          >
        </template>
      </div>

      <h3 class="text-h5 font-weight-bold text-high-emphasis mb-2">
        {{
          searchQuery
            ? $t('media.no_results')
            : filters.favoritesOnly
              ? $t('media.no_favorites')
              : $t('media.your_library_empty')
        }}
      </h3>
      <p class="text-body-1 text-medium-emphasis max-w-400 mx-auto mb-8">
        {{
          searchQuery
            ? $t('media.no_results_for', { query: searchQuery })
            : filters.favoritesOnly
              ? $t('media.tap_heart_hint')
              : $t('media.add_folder_hint')
        }}
      </p>

      <v-btn
        v-if="searchQuery"
        variant="flat"
        class="px-8 py-6"
        color="primary"
        @click="$emit('clear-search')"
      >
        {{ $t('media.clear_search') }}
      </v-btn>
    </div>

    <div id="scroll-sentinel" class="scroll-sentinel"></div>

    <div class="loading-container py-12 d-flex justify-center">
      <v-fade-transition>
        <div v-if="loading" class="d-flex flex-column align-center">
          <v-progress-circular
            indeterminate
            color="rgb(var(--v-theme-on-surface))"
            size="32"
            width="3"
          ></v-progress-circular>
          <span
            class="mt-4 text-caption text-disabled font-weight-medium tracking-widest text-uppercase"
            >{{ $t('media.loading_memories') }}</span
          >
        </div>
        <v-btn
          v-else-if="!allLoaded && groups.length > 0"
          @click="loadFiles"
          variant="outlined"
          class="px-10 py-6"
        >
          {{ $t('media.load_more') }}
        </v-btn>
      </v-fade-transition>
    </div>

    <MediaViewer
      v-model="viewerOpen"
      :photos="images"
      v-model:index="currentPhotoIndex"
      @navigate-to-person="$emit('search-person', $event)"
      @update:photo="handlePhotoUpdated"
    />
    <AddToAlbumSheet
      v-model="addToAlbumOpen"
      :photo-ids="[...selectedIds].map(String)"
      @added="onAddedToAlbum"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { DynamicScroller, DynamicScrollerItem } from 'vue-virtual-scroller';
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css';
import MediaCard from './MediaCard.vue';
import MediaViewer from './MediaViewer.vue';
import AddToAlbumSheet from '@/components/albums/AddToAlbumSheet.vue';
import { useAlbumsStore } from '@/stores/albums';
import { useUiStore } from '@/stores/ui';
import { useSyncStore } from '@/stores/sync';
import { useScanStore } from '@/stores/scan';
import { getPhotosByIds, setFavorites } from '@/services/tauri';
import { useI18n } from 'vue-i18n';
import { useMediaUrl } from '@/composables/useMediaUrl';
import type { MediaItem } from '@/types/media';
import type { FacetType } from '@/types/search';
import type { AlbumSectionItem } from '@/types/albums';

const { t } = useI18n();
const syncStore = useSyncStore();
const scanStore = useScanStore();
const albumsStore = useAlbumsStore();
const uiStore = useUiStore();
const { thumbUrl: buildThumbUrl } = useMediaUrl();

const props = withDefaults(
  defineProps<{
    searchQuery?: string;
    isPersonFilter?: boolean;
    filters?: {
      favoritesOnly: boolean;
      videosOnly: boolean;
      facesOnly: boolean;
      papersOnly: boolean;
      nsfwOnly: boolean;
      camera: string | null;
      aestheticsMin: number | null;
      surprise: boolean;
      orderBy: string;
      personMatch: 'and' | 'or';
      personAlone: boolean;
      dateRange: string;
      folder: string | null;
    };
    facets?: { type: FacetType; value: string; label: string }[];
  }>(),
  {
    searchQuery: '',
    isPersonFilter: false,
    filters: () => ({
      favoritesOnly: false,
      videosOnly: false,
      facesOnly: false,
      papersOnly: false,
      nsfwOnly: false,
      camera: null,
      aestheticsMin: null,
      surprise: false,
      orderBy: 'newest',
      personMatch: 'and',
      personAlone: false,
      dateRange: 'all',
      folder: null,
    }),
    facets: () => [],
  },
);

defineEmits<{
  'clear-search': [];
  'search-person': [person: { id: string; name: string }];
}>();

const loading = ref(false);
const allLoaded = ref(false);
const paging = ref({ offset: 0, limit: 50 });
const images = ref<MediaItem[]>([]);
const imagesMap = ref<Record<string, MediaItem>>({});
const groups = ref<{ name: string; sortKey: string; images: MediaItem[] }[]>([]);
const groupsMap = ref<Record<string, { name: string; sortKey: string; images: MediaItem[] }>>({});
const selectedIds = ref(new Set<string | number>());
const viewerOpen = ref(false);
const currentPhotoIndex = ref(0);
const columns = ref(5);
const addToAlbumOpen = ref(false);

async function handleNotSynced(): Promise<void> {
  await syncStore.reconnect();
}

let observer: IntersectionObserver | null = null;
let unlistenAnalysisResult: UnlistenFn | null = null;
let unlistenPhotoReceived: UnlistenFn | null = null;
let unlistenRefreshed: UnlistenFn | null = null;
let reloadTimer: ReturnType<typeof setTimeout> | null = null;
let photoReceivedTimer: ReturnType<typeof setTimeout> | null = null;
let reloadScrollRestore: { scrollY: number; pages: number } | null = null;
let analysisTimer: ReturnType<typeof setTimeout> | null = null;
let analysisPendingIds = new Set<string | number>();

const isScanning = computed(() => scanStore.isActive);

const trips = computed(() => {
  const section = albumsStore.sections.find((s) => s.id === 'trips');
  return section?.items ?? [];
});

const hasActiveFilters = computed(() => {
  const f = props.filters;
  return (
    !!props.searchQuery ||
    (props.facets?.length ?? 0) > 0 ||
    !!f?.favoritesOnly ||
    !!f?.videosOnly ||
    !!f?.facesOnly ||
    !!f?.papersOnly ||
    !!f?.nsfwOnly ||
    !!f?.camera ||
    f?.aestheticsMin != null ||
    !!f?.surprise ||
    (f?.dateRange ?? 'all') !== 'all' ||
    !!f?.folder
  );
});

const showTrips = computed(() => trips.value.length > 0 && !hasActiveFilters.value);

function tripCoverSrc(trip: AlbumSectionItem): string {
  if (trip.cover_location) return buildThumbUrl(trip.cover_location) ?? '';
  return trip.cover_encoded ?? '';
}

function openTrip(trip: AlbumSectionItem): void {
  void trip;
  uiStore.setPage('albums');
}

function goToTrips(): void {
  uiStore.setPage('albums');
}

const useVirtualScroller = computed(() => {
  return typeof IntersectionObserver !== 'undefined' && virtualItems.value.length > 12;
});

const virtualItems = computed(() => {
  const cols = columns.value;
  const items: Array<{
    type: string;
    key: string;
    name?: string;
    count?: number;
    photos?: MediaItem[];
  }> = [];
  for (const group of groups.value) {
    const name = group.name === 'All photos' ? t('media.all_photos') : group.name;
    items.push({
      type: 'header',
      key: `h-${group.name}`,
      name,
      count: group.images.length,
    });
    for (let i = 0; i < group.images.length; i += cols) {
      items.push({
        type: 'row',
        key: `r-${group.name}-${i}`,
        photos: group.images.slice(i, i + cols),
      });
    }
  }
  return items;
});

function updateColumns(): void {
  const width = window.innerWidth;
  if (width < 640) columns.value = 2;
  else if (width < 1024) columns.value = 3;
  else columns.value = 5;
}

function updateGroups(newImages: MediaItem[]): void {
  const locale = localStorage.getItem('siegu_language') || 'en';
  const sortBy = props.filters?.orderBy ?? 'newest';
  const flat = sortBy === 'best' || sortBy === 'random';
  const affectedGroups = new Set<{ name: string; sortKey: string; images: MediaItem[] }>();

  newImages.forEach((image) => {
    if (imagesMap.value[image.id]) return;
    imagesMap.value[image.id] = image;
    images.value.push(image);

    if (!image._groupKey) {
      if (flat) {
        image._groupKey = 'All photos';
        image._sortKey = '0';
      } else if (image.created) {
        const datePart = image.created.split(' ')[0];
        const dateParts = datePart.includes(':') ? datePart.split(':') : datePart.split('-');
        if (dateParts.length >= 2) {
          const year = dateParts[0];
          const monthIdx = parseInt(dateParts[1]) - 1;
          if (monthIdx >= 0 && monthIdx < 12) {
            const monthName = new Date(parseInt(year), monthIdx).toLocaleString(locale, {
              month: 'long',
            });
            image._groupKey = `${monthName} ${year}`;
            image._sortKey = `${year}${dateParts[1].padStart(2, '0')}`;
          }
        }
      }
      if (!image._groupKey) {
        image._groupKey = 'Recent';
        image._sortKey = '999999';
      }
    }

    let group = groupsMap.value[image._groupKey];
    if (!group) {
      group = { name: image._groupKey, sortKey: image._sortKey ?? '', images: [] };
      groupsMap.value[image._groupKey] = group;
      groups.value.push(group);
      groups.value.sort((a, b) => b.sortKey.localeCompare(a.sortKey));
    }
    group.images.push(image);
    affectedGroups.add(group);
  });

  affectedGroups.forEach((group) => {
    if (flat) return;
    group.images.sort((a, b) => (b.created || '').localeCompare(a.created || ''));
  });

  groups.value = [...groups.value];
}

function handlePhotoUpdated(updatedPhoto: MediaItem): void {
  const existing = imagesMap.value[updatedPhoto.id];
  if (existing) {
    Object.assign(existing, updatedPhoto);
  } else {
    updateGroups([updatedPhoto]);
  }
}

// Analysis results arrive as one event per photo, which used to trigger one
// DB round-trip per event (an IPC storm during a batch index). Coalesce them:
// pending ids are flushed together every ~300ms (or when the buffer fills),
// mutating items in place instead of rebuilding the whole group array.
function queueAnalysisResult(id: string | number): void {
  analysisPendingIds.add(id);
  if (analysisPendingIds.size >= 50) {
    void flushAnalysisResults();
    return;
  }
  if (!analysisTimer) {
    analysisTimer = setTimeout(() => void flushAnalysisResults(), 300);
  }
}

async function flushAnalysisResults(): Promise<void> {
  if (analysisPendingIds.size === 0) {
    analysisTimer = null;
    return;
  }
  const ids = [...analysisPendingIds];
  analysisPendingIds = new Set();
  analysisTimer = null;
  try {
    const updatedPhotos = await getPhotosByIds(ids);
    const fresh: MediaItem[] = [];
    for (const updated of updatedPhotos) {
      const existing = imagesMap.value[updated.id];
      if (existing) {
        updated._groupKey = existing._groupKey;
        updated._sortKey = existing._sortKey;
        imagesMap.value[updated.id] = updated;
        const idx = images.value.findIndex((p) => p.id === updated.id);
        if (idx !== -1) images.value[idx] = updated;
        for (const g of groups.value) {
          const gi = g.images.findIndex((p) => p.id === updated.id);
          if (gi !== -1) g.images[gi] = updated;
        }
      } else {
        fresh.push(updated);
      }
    }
    if (fresh.length > 0) updateGroups(fresh);
  } catch (e) {
    console.warn('Failed to fetch updated photos after analysis:', e);
  }
}

function toggleSelection(id: string | number): void {
  const set = selectedIds.value;
  if (set.has(id)) set.delete(id);
  else set.add(id);
}

function clearSelection(): void {
  selectedIds.value.clear();
}

async function bulkFavorite(): Promise<void> {
  const ids = [...selectedIds.value];
  if (ids.length === 0) return;
  try {
    await setFavorites(ids, true);
    for (const id of ids) {
      const photo = imagesMap.value[id];
      if (photo) photo.favorite = true;
    }
  } catch (error) {
    console.error('Failed to bulk favorite:', error);
  }
  clearSelection();
}

function bulkRemove(): void {
  clearSelection();
}

function onAddedToAlbum(albumName: string): void {
  void albumName;
  clearSelection();
}

function setupInfiniteScroll(): void {
  if (typeof IntersectionObserver === 'undefined') return;
  observer = new IntersectionObserver(
    (entries) => {
      if (entries[0].isIntersecting && !loading.value && !allLoaded.value) {
        loadFiles();
      }
    },
    { threshold: 0.01, rootMargin: '600px' },
  );
  const sentinel = document.getElementById('scroll-sentinel');
  if (sentinel) observer.observe(sentinel);
}

async function loadFiles(): Promise<void> {
  if (loading.value) return;
  loading.value = true;
  try {
    let response: string;
    if (props.isPersonFilter && props.searchQuery) {
      response = await invoke<string>('get_person_photos', {
        personId: props.searchQuery,
        offset: paging.value.offset,
        limit: paging.value.limit,
      });
    } else {
      const byType = (type: string) => props.facets?.find((f) => f.type === type);
      const people = props.facets?.filter((f) => f.type === 'person') ?? [];
      const personIds = people.map((f) => f.value);
      const location = byType('location');
      const tag = byType('tag');
      const month = byType('month');
      const date = byType('date');
      const dateRange = date ? (date.value.split('|') as [string, string]) : null;
      response = await invoke<string>('list_files', {
        offset: paging.value.offset,
        limit: paging.value.limit,
        query: props.searchQuery ?? '',
        scan: false,
        favoritesOnly: props.filters?.favoritesOnly ?? false,
        videosOnly: props.filters?.videosOnly ?? false,
        personIds: personIds.length ? personIds : null,
        personMatch: props.filters?.personMatch ?? 'and',
        personAlone: props.filters?.personAlone ?? false,
        location: location ? location.value : null,
        tag: tag ? tag.value : null,
        dateFrom: month ? `${month.value}-01` : dateRange ? dateRange[0] : null,
        dateTo: month ? `${month.value}-31` : dateRange ? dateRange[1] : null,
        hasFaces: props.filters?.facesOnly ?? false,
        papers: props.filters?.papersOnly ?? false,
        nsfwOnly: props.filters?.nsfwOnly ?? false,
        camera: props.filters?.camera ?? null,
        aestheticsMin: props.filters?.aestheticsMin ?? null,
        random: props.filters?.surprise ?? false,
        orderBy: props.filters?.orderBy ?? null,
      });
    }

    const newImages: MediaItem[] = JSON.parse(response);

    if (paging.value.offset === 0) {
      imagesMap.value = {};
      groupsMap.value = {};
      groups.value = [];
      images.value = [];
      analysisPendingIds.clear();
      updateGroups(newImages);
    } else {
      updateGroups(newImages);
    }

    if (newImages.length < paging.value.limit) {
      allLoaded.value = true;
    } else {
      paging.value.offset += paging.value.limit;
    }
  } catch (err) {
    console.error('Failed to list files:', err);
  } finally {
    loading.value = false;
  }
}

function scheduleReload(options?: { preserveScroll?: boolean }): void {
  if (reloadTimer) clearTimeout(reloadTimer);
  // Background refreshes (new photos arriving mid-browse) keep the user's spot;
  // explicit filter/search changes reset to the top.
  reloadScrollRestore = options?.preserveScroll
    ? { scrollY: window.scrollY, pages: paging.value.offset }
    : null;
  reloadTimer = setTimeout(async () => {
    loading.value = false;
    paging.value.offset = 0;
    allLoaded.value = false;
    await loadFiles();
    const restore = reloadScrollRestore;
    if (restore) {
      // Reload page 1 through the previous page depth so the full list height
      // is available again, then restore the physical scroll offset.
      while (paging.value.offset < restore.pages && !allLoaded.value) {
        await loadFiles();
      }
      window.scrollTo(0, restore.scrollY);
      reloadScrollRestore = null;
    }
  }, 200);
}

async function handleToggleFavorite(id: string | number): Promise<void> {
  try {
    const isNowFavorite = await invoke<boolean>('toggle_favorite', { id });
    const photo = imagesMap.value[id];
    if (photo) {
      photo.favorite = isNowFavorite;
      if (props.filters?.favoritesOnly && !isNowFavorite) {
        images.value = images.value.filter((p) => p.id !== id);
        delete imagesMap.value[id];
        if (photo._groupKey) {
          const group = groupsMap.value[photo._groupKey];
          if (group) {
            group.images = group.images.filter((p: MediaItem) => p.id !== id);
            if (group.images.length === 0) {
              delete groupsMap.value[photo._groupKey];
              groups.value = groups.value.filter((g) => g.name !== photo._groupKey);
            }
          }
        }
        selectedIds.value.delete(id);
      }
    }
  } catch (err) {
    console.error('Failed to toggle favorite:', err);
  }
}

function openViewer(index: number): void {
  currentPhotoIndex.value = index;
  viewerOpen.value = true;
}

function openViewerByPhoto(photo: MediaItem): void {
  const index = images.value.findIndex((p) => p.id === photo.id);
  if (index !== -1) openViewer(index);
}

watch(
  () => props.searchQuery,
  () => {
    scheduleReload();
  },
);

watch(
  () => props.filters,
  () => {
    scheduleReload();
  },
  { deep: true },
);

watch(
  () => props.facets,
  () => {
    scheduleReload();
  },
  { deep: true },
);

onMounted(async () => {
  loadFiles();

  // During a bulk index the backend throttles per-photo results, but a stray
  // result (e.g. a late thumbnail) must not start a `getPhotosByIds` storm
  // while indexing is still churning. Single-photo manual analysis keeps
  // flowing because it only ever produces one event at a time.
  unlistenAnalysisResult = await listen<{ id: string | number }>(
    'photo-analysis-result',
    (event) => {
      const id = event.payload.id;
      if (!id) return;
      if (scanStore.status === 'indexing' || scanStore.indexingCount > 0) return;
      queueAnalysisResult(id);
    },
  );

  unlistenPhotoReceived = await listen<MediaItem>('photo-received', (event) => {
    if (event.payload?.id) {
      updateGroups([event.payload]);
    }
    if (photoReceivedTimer) clearTimeout(photoReceivedTimer);
    photoReceivedTimer = setTimeout(() => scheduleReload({ preserveScroll: true }), 500);
  });

  unlistenRefreshed = await listen('photos-refreshed', () => {
    scheduleReload({ preserveScroll: true });
    void albumsStore.loadSections();
  });

  void albumsStore.loadSections();
  updateColumns();
  window.addEventListener('resize', updateColumns);
  setupInfiniteScroll();
});

onUnmounted(() => {
  window.removeEventListener('resize', updateColumns);
  observer?.disconnect();
  unlistenAnalysisResult?.();
  unlistenPhotoReceived?.();
  unlistenRefreshed?.();
  if (reloadTimer) clearTimeout(reloadTimer);
  if (photoReceivedTimer) clearTimeout(photoReceivedTimer);
  if (analysisTimer) clearTimeout(analysisTimer);
});
</script>

<style scoped>
.media-library-container {
  min-height: 100vh;
}

.photo-row {
  display: grid;
  gap: 16px;
  padding-bottom: 16px;
}

.month-header {
  position: sticky;
  top: 64px;
  z-index: 10;
}

.header-blur {
  background: color-mix(in srgb, rgb(var(--v-theme-background)) 80%, transparent);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
}

.letter-spacing-tight {
  letter-spacing: -0.02em;
}

.bulk-toolbar-container {
  position: fixed;
  bottom: 110px;
  left: 0;
  right: 0;
  display: flex;
  justify-content: center;
  z-index: 2100;
  padding: 0 24px;
}

.bulk-toolbar {
  width: 100%;
  max-width: 560px;
  box-shadow:
    0 20px 25px -5px rgba(0, 0, 0, 0.1),
    0 10px 10px -5px rgba(0, 0, 0, 0.04);
}

.empty-state-container {
  min-height: 60vh;
}

.max-w-400 {
  max-width: 400px;
}

.animate-fade-in {
  animation: fadeIn 0.6s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.scroll-sentinel {
  height: 20px;
}

.section-title {
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-size: 12px;
}

.trips-scroll {
  display: flex;
  gap: 16px;
  overflow-x: auto;
  padding-bottom: 8px;
  scrollbar-width: thin;
}

.trip-card {
  flex: 0 0 200px;
  cursor: pointer;
  transition: transform 0.2s ease;
}

.trip-card:hover {
  transform: translateY(-2px);
}

.trip-cover {
  position: relative;
  aspect-ratio: 1;
  border-radius: 20px;
  overflow: hidden;
  background: rgb(var(--v-theme-surface-light));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
}

.trip-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.trip-cover-placeholder {
  width: 100%;
  height: 100%;
}

.trip-count {
  position: absolute;
  bottom: 8px;
  left: 8px;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 9999px;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 11px;
  font-weight: 700;
  backdrop-filter: blur(4px);
}

.trip-name {
  margin-top: 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
