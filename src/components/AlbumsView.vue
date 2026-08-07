<template>
  <div class="albums-container px-4 py-6">
    <v-fade-transition>
      <div v-if="isManualAlbum && selectedIds.length > 0" class="bulk-toolbar-container">
        <v-sheet
          class="bulk-toolbar d-flex align-center px-6 py-3 rounded-pill shadow-xl"
          color="var(--color-bg-btn)"
        >
          <v-btn
            icon="mdi-close"
            variant="text"
            density="comfortable"
            color="var(--color-text-btn)"
            @click="clearSelection"
          ></v-btn>
          <div class="ml-4">
            <div class="text-subtitle-2 font-weight-bold text-btn">
              {{ $t('albums.selected', { count: selectedIds.length }) }}
            </div>
          </div>
          <v-spacer></v-spacer>
          <v-btn
            variant="flat"
            color="var(--color-bg-surface)"
            class="text-btn px-6 rounded-xl text-none font-weight-bold"
            size="small"
            @click="bulkRemoveFromAlbum"
          >
            <v-icon size="16" class="mr-2">mdi-minus-circle-outline</v-icon>
            {{ $t('albums.remove_from_album') }}
          </v-btn>
        </v-sheet>
      </div>
    </v-fade-transition>

    <template v-if="!currentSectionItem">
      <div class="d-flex align-center px-2 mb-4">
        <div>
          <h1 class="text-h5 font-weight-bold text-zinc-primary letter-spacing-tight">
            {{ $t('albums.title') }}
          </h1>
          <p class="text-caption text-zinc-muted">{{ $t('albums.desc') }}</p>
        </div>
        <v-spacer></v-spacer>
        <v-btn
          variant="text"
          class="mr-1"
          :title="$t('albums.restore_trips')"
          :aria-label="$t('albums.restore_trips')"
          @click="restoreTrips"
        >
          <v-icon size="18">mdi-undo</v-icon>
        </v-btn>
        <v-btn
          variant="flat"
          color="primary"
          class="siegu-btn-modern px-6"
          @click="openNewAlbumDialog"
        >
          <v-icon start size="18">mdi-plus</v-icon>
          {{ $t('albums.new_album') }}
        </v-btn>
      </div>

      <template v-if="hasAnyItems">
        <div v-for="section in sections" :key="section.id" class="mb-8">
          <div v-if="section.items.length > 0" class="animate-fade-in">
            <div class="d-flex align-center px-2 mb-3">
              <h2
                class="section-title text-subtitle-1 font-weight-bold text-zinc-primary flex-grow-1"
              >
                {{ sectionTitle(section.id) }}
              </h2>
              <button
                v-if="section.id === 'people'"
                class="manage-people-btn"
                @click="togglePeopleManage"
              >
                <v-icon size="13" class="mr-1">mdi-account-edit-outline</v-icon>
                {{ $t('albums.manage_people') }}
              </button>
            </div>
            <div class="album-grid" :style="{ gridTemplateColumns: `repeat(${columns}, 1fr)` }">
              <div
                v-for="item in section.items"
                :key="item.id"
                class="album-card"
                @click="openSectionItem(item)"
              >
                <div class="album-cover">
                  <img
                    v-if="tileSrc(item)"
                    :src="tileSrc(item)"
                    :alt="item.name"
                    loading="lazy"
                    class="album-cover-img"
                  />
                  <div v-else class="album-cover-placeholder d-flex align-center justify-center">
                    <v-icon size="44" color="var(--color-icon-empty)">{{
                      tileIcon(item.kind)
                    }}</v-icon>
                  </div>
                  <div class="album-count">
                    <v-icon size="12">mdi-image</v-icon>
                    {{ $t('albums.items_count', { count: item.count }) }}
                  </div>
                </div>
                <div class="album-name text-subtitle-2 font-weight-bold text-zinc-primary">
                  {{ item.name }}
                </div>
              </div>
            </div>
            <div v-if="section.id === 'people' && peopleManageOpen" class="mt-6">
              <PeopleManagePanel
                :faces="unnamedFaces"
                :indexing-count="indexingCount"
                @start-indexing="startPeopleIndexing"
                @view-cluster="handleViewCluster"
                @prompt-name="promptName"
              />
            </div>
          </div>
        </div>
        <div v-if="showPeopleManageFallback" class="mb-8">
          <PeopleManagePanel
            :faces="unnamedFaces"
            :indexing-count="indexingCount"
            @start-indexing="startPeopleIndexing"
            @view-cluster="handleViewCluster"
            @prompt-name="promptName"
          />
        </div>
      </template>

      <div
        v-else-if="!albumsStore.sectionsLoading"
        class="empty-state-container d-flex flex-column align-center justify-center text-center"
      >
        <div class="empty-state-icon mb-6">
          <v-icon size="80" color="var(--color-icon-empty)">mdi-image-album</v-icon>
        </div>
        <h3 class="text-h5 font-weight-bold text-zinc-primary mb-2">
          {{ $t('albums.no_albums') }}
        </h3>
        <p class="text-body-1 text-zinc-secondary max-w-400 mx-auto mb-8">
          {{ $t('albums.no_albums_hint') }}
        </p>
        <v-btn variant="flat" class="siegu-btn-modern px-8 py-6" @click="openNewAlbumDialog">
          {{ $t('albums.new_album') }}
        </v-btn>
      </div>
    </template>

    <template v-else>
      <div class="d-flex align-center px-2 mb-2">
        <v-btn icon variant="text" size="small" :aria-label="$t('albums.back')" @click="closeAlbum">
          <v-icon size="20">mdi-arrow-left</v-icon>
        </v-btn>
        <div class="ml-2">
          <h1 class="text-h6 font-weight-bold text-zinc-primary letter-spacing-tight">
            {{ currentSectionItem?.name }}
          </h1>
          <p class="text-caption text-zinc-muted">
            <template v-if="currentSectionItem">
              {{ kindLabel(currentSectionItem.kind) }}
              <span class="mx-1">·</span>
            </template>
            {{ $t('albums.items_count', { count: items.length }) }}
          </p>
        </div>
        <v-spacer></v-spacer>
        <v-menu v-if="currentSectionItem?.album || currentSectionItem?.kind === 'person'">
          <template v-slot:activator="{ props: menuProps }">
            <v-btn v-bind="menuProps" icon variant="text" size="small">
              <v-icon size="20">mdi-dots-vertical</v-icon>
            </v-btn>
          </template>
          <v-list density="compact" class="siegu-list">
            <v-list-item
              v-if="currentSectionItem?.kind === 'person'"
              @click="openManagePerson"
              prepend-icon="mdi-account-cog-outline"
            >
              <v-list-item-title>{{ $t('people.profile_actions') }}</v-list-item-title>
            </v-list-item>
            <template v-if="currentSectionItem?.album">
              <v-list-item
                v-if="currentSectionItem?.album?.kind === 'smart'"
                @click="editSmartAlbumRules"
                prepend-icon="mdi-tune-variant"
              >
                <v-list-item-title>{{ $t('albums.edit_rules') }}</v-list-item-title>
              </v-list-item>
              <v-list-item @click="openRenameDialog" prepend-icon="mdi-pencil-outline">
                <v-list-item-title>{{ $t('albums.rename_album') }}</v-list-item-title>
              </v-list-item>
              <v-list-item @click="confirmDelete = true" prepend-icon="mdi-delete-outline">
                <v-list-item-title class="text-error">{{
                  $t('albums.delete_album')
                }}</v-list-item-title>
              </v-list-item>
            </template>
          </v-list>
        </v-menu>
      </div>

      <div v-if="isManualAlbum" class="px-2 mb-3" style="max-width: 480px">
        <v-text-field
          v-model="query"
          :label="$t('albums.search_in_album')"
          variant="outlined"
          density="comfortable"
          hide-details
          clearable
          prepend-inner-icon="mdi-magnify"
        ></v-text-field>
      </div>

      <div
        v-if="isManualAlbum && !searching && items.length > 0"
        class="text-caption text-zinc-muted px-2 mb-2"
      >
        {{ $t('albums.reorder_hint') }}
      </div>

      <DynamicScroller
        v-if="displayItems.length > 0 && useVirtualScroller"
        class="animate-fade-in"
        :items="virtualItems"
        :min-item-size="240"
        key-field="key"
        page-mode
        v-slot="{ item, active }"
      >
        <DynamicScrollerItem :item="item" :active="active">
          <div
            class="photo-row"
            :style="{ gridTemplateColumns: `repeat(${columns}, 1fr)` }"
            @dragover.prevent
            @drop="onDrop"
          >
            <div
              v-for="(photo, i) in item.photos"
              :key="photo.id"
              class="drag-item"
              :class="{ 'drag-over': dragIndex === item.startIndex + i }"
              :draggable="isManualAlbum && !selectionActive"
              @dragstart="onDragStart(photo, item.startIndex + i)"
              @dragover.prevent="onDragOver(item.startIndex + i)"
              @dragend="onDragEnd"
            >
              <MediaCard
                :path="photo"
                :selected="selectedIds.includes(photo.id)"
                :selection-mode="selectedIds.length > 0"
                @click="openViewer(item.startIndex + i)"
                @select="toggleSelection"
                @toggle-favorite="handleToggleFavorite"
              />
            </div>
          </div>
        </DynamicScrollerItem>
      </DynamicScroller>

      <div
        v-else
        class="photo-row"
        :style="{ gridTemplateColumns: `repeat(${columns}, 1fr)` }"
        @dragover.prevent
        @drop="onDrop"
      >
        <div
          v-for="(photo, index) in displayItems"
          :key="photo.id"
          class="drag-item"
          :class="{ 'drag-over': dragIndex === index }"
          :draggable="isManualAlbum && !selectionActive"
          @dragstart="onDragStart(photo, index)"
          @dragover.prevent="onDragOver(index)"
          @dragend="onDragEnd"
        >
          <MediaCard
            :path="photo"
            :selected="selectedIds.includes(photo.id)"
            :selection-mode="selectedIds.length > 0"
            @click="openViewer(index)"
            @select="toggleSelection"
            @toggle-favorite="handleToggleFavorite"
          />
        </div>
      </div>

      <div
        v-if="!searching && items.length === 0 && !loadingContents"
        class="empty-state-container d-flex flex-column align-center justify-center text-center"
      >
        <div class="empty-state-icon mb-6">
          <v-icon size="80" color="var(--color-icon-empty)">mdi-image-outline</v-icon>
        </div>
        <h3 class="text-h5 font-weight-bold text-zinc-primary mb-2">
          {{ $t('albums.empty_album') }}
        </h3>
        <p class="text-body-1 text-zinc-secondary max-w-400 mx-auto mb-8">
          {{ $t('albums.empty_album_hint') }}
        </p>
      </div>

      <div
        v-else-if="searching && displayItems.length === 0 && !searchLoading"
        class="empty-state-container d-flex flex-column align-center justify-center text-center"
      >
        <div class="empty-state-icon mb-6">
          <v-icon size="80" color="var(--color-icon-empty)">mdi-text-search-variant</v-icon>
        </div>
        <h3 class="text-h5 font-weight-bold text-zinc-primary mb-2">
          {{ $t('albums.no_results_in_album', { query }) }}
        </h3>
      </div>

      <div class="loading-container py-8 d-flex justify-center">
        <v-fade-transition>
          <div v-if="loadingContents || searchLoading" class="d-flex align-center">
            <v-progress-circular
              indeterminate
              color="var(--color-text-primary)"
              size="28"
              width="3"
            ></v-progress-circular>
          </div>
          <v-btn
            v-else-if="!searching && !allLoaded && items.length > 0"
            @click="loadMore"
            variant="flat"
            class="siegu-btn-outline px-10 py-6"
          >
            {{ $t('albums.load_more') }}
          </v-btn>
        </v-fade-transition>
      </div>

      <MediaViewer
        v-model="viewerOpen"
        :photos="displayItems"
        v-model:index="currentPhotoIndex"
        @update:photo="handlePhotoUpdated"
      />
    </template>

    <v-dialog v-model="newAlbumDialog" max-width="420">
      <v-card class="rounded-xl pa-6" color="surface">
        <h3 class="text-h6 font-weight-bold text-zinc-primary mb-4">
          {{ $t('albums.new_album') }}
        </h3>
        <v-text-field
          v-model="newAlbumName"
          :label="$t('albums.new_album_placeholder')"
          variant="outlined"
          hide-details
          @keyup.enter="createAlbum"
        ></v-text-field>
        <div class="d-flex justify-end mt-4 ga-2">
          <v-btn variant="text" @click="newAlbumDialog = false">{{ $t('common.cancel') }}</v-btn>
          <v-btn
            variant="flat"
            color="primary"
            class="siegu-btn-modern px-6"
            :disabled="!newAlbumName.trim()"
            :loading="creating"
            @click="createAlbum"
          >
            {{ $t('common.create') }}
          </v-btn>
        </div>
      </v-card>
    </v-dialog>

    <v-dialog v-model="renameDialog" max-width="420">
      <v-card class="rounded-xl pa-6" color="surface">
        <h3 class="text-h6 font-weight-bold text-zinc-primary mb-4">
          {{ $t('albums.rename_album') }}
        </h3>
        <v-text-field
          v-model="renameName"
          :label="$t('albums.rename_placeholder')"
          variant="outlined"
          hide-details
          @keyup.enter="renameCurrentAlbum"
        ></v-text-field>
        <div class="d-flex justify-end mt-4 ga-2">
          <v-btn variant="text" @click="renameDialog = false">{{ $t('common.cancel') }}</v-btn>
          <v-btn
            variant="flat"
            color="primary"
            class="siegu-btn-modern px-6"
            :disabled="!renameName.trim()"
            @click="renameCurrentAlbum"
          >
            {{ $t('common.save') }}
          </v-btn>
        </div>
      </v-card>
    </v-dialog>

    <v-dialog v-model="confirmDelete" max-width="420">
      <v-card class="rounded-xl pa-6" color="surface">
        <h3 class="text-h6 font-weight-bold text-zinc-primary mb-2">
          {{ $t('albums.delete_confirm_title') }}
        </h3>
        <p class="text-body-2 text-zinc-secondary mb-4">{{ $t('albums.delete_confirm') }}</p>
        <div class="d-flex justify-end ga-2">
          <v-btn variant="text" @click="confirmDelete = false">{{ $t('common.cancel') }}</v-btn>
          <v-btn variant="flat" color="error" class="px-6" @click="deleteCurrentAlbum">
            {{ $t('common.delete') }}
          </v-btn>
        </div>
      </v-card>
    </v-dialog>

    <NameDialog
      v-model="nameDialog"
      :active-face="activeFace"
      :people="people"
      @save="handleSaveName"
    />

    <ManageDialog
      v-model="manageDialog"
      :active-person="activePerson"
      :people="people"
      @rename="handleRenamePerson"
      @merge="handleMergePerson"
    />

    <ClusterDialog
      v-model="clusterDialog"
      :cluster="activeCluster"
      :faces="clusterFaces"
      @remove-face="handleRemoveFace"
      @prompt-name="promptNameFromCluster"
    />

    <v-snackbar v-model="snackbar" timeout="2500" color="surface" location="bottom">
      <span class="text-body-2 text-zinc-primary">{{ snackbarText }}</span>
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { DynamicScroller, DynamicScrollerItem } from 'vue-virtual-scroller';
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css';
import { useI18n } from 'vue-i18n';
import MediaCard from '@/components/MediaCard.vue';
import MediaViewer from '@/components/MediaViewer.vue';
import PeopleManagePanel from '@/components/people/PeopleManagePanel.vue';
import NameDialog from '@/components/people/NameDialog.vue';
import ManageDialog from '@/components/people/ManageDialog.vue';
import ClusterDialog from '@/components/people/ClusterDialog.vue';
import { useAlbumsStore } from '@/stores/albums';
import { useSearchStore } from '@/stores/search';
import { useUiStore } from '@/stores/ui';
import { usePeople } from '@/composables/usePeople';
import { toggleFavorite, listFiles } from '@/services/tauri';
import { getFaceImageSrc } from '@/composables/useMediaUtils';
import type { Album, AlbumSectionItem } from '@/types/albums';
import type { MediaItem } from '@/types/media';
import type { ListFilesOptions } from '@/types/media';
import type { Person, UnnamedFace } from '@/types/person';

const { t } = useI18n();
const albumsStore = useAlbumsStore();
const searchStore = useSearchStore();
const uiStore = useUiStore();
const {
  people,
  unnamedFaces,
  indexingCount,
  fetchData: fetchPeopleData,
  startIndexing: startPeopleIndexing,
  saveName: savePeopleName,
  renamePersonById,
  mergePersonById,
  fetchClusterFaces,
  removeFace,
} = usePeople();

const sections = computed(() => albumsStore.sections);
const hasAnyItems = computed(() => sections.value.some((s) => s.items.length > 0));
const openedItem = ref<AlbumSectionItem | null>(null);
const currentSectionItem = computed(() => openedItem.value);
const isManualAlbum = computed(() => openedItem.value?.kind === 'manual');

const PAGE_SIZE = 60;
const items = ref<MediaItem[]>([]);
const searchResults = ref<MediaItem[]>([]);
const itemsMap = ref<Record<string, MediaItem>>({});
const loadingContents = ref(false);
const searchLoading = ref(false);
const allLoaded = ref(false);
const offset = ref(0);
const query = ref('');
const viewerOpen = ref(false);
const currentPhotoIndex = ref(0);
const selectedIds = ref<(string | number)[]>([]);
const columns = ref(3);

const newAlbumDialog = ref(false);
const newAlbumName = ref('');
const creating = ref(false);
const renameDialog = ref(false);
const renameName = ref('');
const confirmDelete = ref(false);
const snackbar = ref(false);
const snackbarText = ref('');

const peopleManageOpen = ref(false);
const nameDialog = ref(false);
const manageDialog = ref(false);
const clusterDialog = ref(false);
const activeFace = ref<Person | null>(null);
const activePerson = ref<Person | null>(null);
const activeCluster = ref<Person | null>(null);
const clusterFaces = ref<UnnamedFace[]>([]);
const hasPeopleSection = computed(() => sections.value.some((s) => s.id === 'people'));
const showPeopleManageFallback = computed(
  () => !hasPeopleSection.value && (unnamedFaces.value.length > 0 || indexingCount.value > 0),
);

const searching = computed(() => query.value.trim().length > 0);
const selectionActive = computed(() => selectedIds.value.length > 0);
const displayItems = computed(() => (searching.value ? searchResults.value : items.value));

const useVirtualScroller = computed(
  () => typeof IntersectionObserver !== 'undefined' && displayItems.value.length > 48,
);

const virtualItems = computed(() => {
  const cols = columns.value;
  const rows: Array<{ type: 'row'; key: string; startIndex: number; photos: MediaItem[] }> = [];
  for (let i = 0; i < displayItems.value.length; i += cols) {
    rows.push({
      type: 'row',
      key: `r-${i}-${displayItems.value[i]?.id}`,
      startIndex: i,
      photos: displayItems.value.slice(i, i + cols),
    });
  }
  return rows;
});

const dragId = ref<string | null>(null);
const dragIndex = ref<number | null>(null);

function showMessage(message: string): void {
  snackbarText.value = message;
  snackbar.value = true;
}

function sectionTitle(sectionId: string): string {
  return t(`albums.section_${sectionId}`);
}

function tileSrc(item: AlbumSectionItem): string {
  if (item.kind === 'person') return getFaceImageSrc(item.cover_crop, item.cover_encoded);
  return item.cover_encoded ?? '';
}

function tileIcon(kind: string): string {
  switch (kind) {
    case 'person':
      return 'mdi-account-circle';
    case 'trip':
      return 'mdi-airplane';
    case 'smart':
      return 'mdi-auto-fix';
    default:
      return 'mdi-image-multiple-outline';
  }
}

function kindLabel(kind: string): string {
  switch (kind) {
    case 'person':
      return t('albums.kind_person');
    case 'trip':
      return t('albums.kind_trip');
    case 'smart':
      return t('albums.kind_smart');
    default:
      return t('albums.kind_album');
  }
}

function resetContents(): void {
  items.value = [];
  itemsMap.value = {};
  searchResults.value = [];
  offset.value = 0;
  allLoaded.value = false;
  query.value = '';
  clearSelection();
}

function openAlbum(album: Album): void {
  openedItem.value = {
    id: album.id,
    name: album.name,
    count: album.item_count,
    cover_encoded: null,
    cover_crop: null,
    kind: album.kind,
    album,
  };
  albumsStore.currentAlbumId = album.id;
  resetContents();
  void loadContents();
}

function openSectionItem(item: AlbumSectionItem): void {
  openedItem.value = item;
  albumsStore.currentAlbumId = item.id;
  resetContents();
  void loadContents();
}

function closeAlbum(): void {
  openedItem.value = null;
  albumsStore.currentAlbumId = null;
  resetContents();
}

function editSmartAlbumRules(): void {
  const album = openedItem.value?.album;
  if (!album || album.kind !== 'smart' || !album.rule) return;
  try {
    const rule = JSON.parse(album.rule) as Record<string, unknown>;
    void searchStore.applyRule(rule);
  } catch (error) {
    console.error('[Albums] Failed to parse smart album rule:', error);
    return;
  }
  albumsStore.startEditingSmartAlbum(album);
  uiStore.setPage('home');
  closeAlbum();
}

async function restoreTrips(): Promise<void> {
  await albumsStore.clearDismissedTrips();
  showMessage(t('albums.trips_restored'));
}

function togglePeopleManage(): void {
  peopleManageOpen.value = !peopleManageOpen.value;
  if (peopleManageOpen.value) void fetchPeopleData();
}

function isPersonItem(item: AlbumSectionItem): boolean {
  return item.kind === 'person' && item.id.startsWith('person:');
}

function openManagePerson(): void {
  const item = openedItem.value;
  if (!item || !isPersonItem(item)) return;
  const personId = Number(item.id.slice('person:'.length));
  activePerson.value = people.value.find((p) => p.id === personId) ?? null;
  manageDialog.value = true;
}

function promptName(group: Person): void {
  activeFace.value = group;
  nameDialog.value = true;
}

function promptNameFromCluster(): void {
  if (activeCluster.value) promptName(activeCluster.value);
}

async function handleViewCluster(group: Person): Promise<void> {
  activeCluster.value = group;
  clusterFaces.value = await fetchClusterFaces(group.id);
  clusterDialog.value = true;
}

async function handleSaveName(faceId: number, name: string): Promise<void> {
  const ok = await savePeopleName(faceId, name);
  if (ok) {
    nameDialog.value = false;
    clusterDialog.value = false;
    await albumsStore.loadSections();
  }
}

async function handleRenamePerson(id: number, newName: string): Promise<void> {
  await renamePersonById(id, newName);
  manageDialog.value = false;
  if (openedItem.value) openedItem.value.name = newName;
  await albumsStore.loadSections();
}

async function handleMergePerson(fromId: number, toId: number): Promise<void> {
  await mergePersonById(fromId, toId);
  manageDialog.value = false;
  await albumsStore.loadSections();
}

async function handleRemoveFace(faceId: number): Promise<void> {
  const ok = await removeFace(faceId);
  if (ok) {
    clusterFaces.value = clusterFaces.value.filter((f) => f.face_id !== faceId);
    if (clusterFaces.value.length === 0) clusterDialog.value = false;
    await fetchPeopleData();
  }
}

function baseFilterOptions(): Omit<ListFilesOptions, 'offset' | 'limit'> {
  const item = openedItem.value;
  if (!item) return {};
  if (item.kind === 'person' && item.id.startsWith('person:')) {
    return { personIds: [item.id.slice('person:'.length)], personMatch: 'and' };
  }
  return { albumId: item.id };
}

async function loadContents(): Promise<void> {
  const item = openedItem.value;
  if (!item) return;
  loadingContents.value = true;
  try {
    let photos: MediaItem[];
    if (item.kind === 'person') {
      photos = await listFiles({
        offset: offset.value,
        limit: PAGE_SIZE,
        ...baseFilterOptions(),
      });
    } else {
      photos = await albumsStore.loadContents(item.id, offset.value, PAGE_SIZE);
    }
    for (const photo of photos) {
      itemsMap.value[String(photo.id)] = photo;
    }
    items.value = [...items.value, ...photos];
    offset.value += photos.length;
    allLoaded.value = photos.length < PAGE_SIZE;
  } catch (error) {
    console.error('[Albums] Failed to load contents:', error);
  } finally {
    loadingContents.value = false;
  }
}

function loadMore(): void {
  void loadContents();
}

let searchTimer: ReturnType<typeof setTimeout> | null = null;
watch(query, (value) => {
  if (searchTimer) clearTimeout(searchTimer);
  const albumId = albumsStore.currentAlbumId;
  if (!albumId) return;
  if (!value.trim()) {
    searchResults.value = [];
    return;
  }
  searchTimer = setTimeout(() => {
    void runSearch(albumId, value);
  }, 300);
});

async function runSearch(albumId: string, value: string): Promise<void> {
  searchLoading.value = true;
  try {
    const photos = await listFiles({
      offset: 0,
      limit: 200,
      query: value,
      albumId,
      orderBy: 'newest',
    });
    searchResults.value = photos;
  } catch (error) {
    console.error('[Albums] Failed to search album:', error);
  } finally {
    searchLoading.value = false;
  }
}

function toggleSelection(id: string | number): void {
  const index = selectedIds.value.indexOf(id);
  if (index === -1) selectedIds.value.push(id);
  else selectedIds.value.splice(index, 1);
}

function clearSelection(): void {
  selectedIds.value = [];
}

async function bulkRemoveFromAlbum(): Promise<void> {
  const albumId = albumsStore.currentAlbumId;
  if (!albumId) return;
  const ids = selectedIds.value.map(String);
  await albumsStore.removeItems(albumId, ids);
  items.value = items.value.filter((photo) => !ids.includes(String(photo.id)));
  searchResults.value = searchResults.value.filter((photo) => !ids.includes(String(photo.id)));
  showMessage(t('albums.removed_from_album', { count: ids.length }));
  clearSelection();
}

function openViewer(index: number): void {
  if (selectionActive.value) return;
  currentPhotoIndex.value = index;
  viewerOpen.value = true;
}

async function handleToggleFavorite(id: string | number): Promise<void> {
  try {
    const isNowFavorite = await toggleFavorite(id as number);
    const photo = itemsMap.value[String(id)];
    if (photo) photo.favorite = isNowFavorite;
  } catch (err) {
    console.error('Failed to toggle favorite:', err);
  }
}
function handlePhotoUpdated(updatedPhoto: MediaItem): void {
  const existing = itemsMap.value[String(updatedPhoto.id)];
  if (existing) {
    Object.assign(existing, updatedPhoto);
  }
}

function onDragStart(photo: MediaItem, index: number): void {
  dragId.value = String(photo.id);
  dragIndex.value = index;
}

function onDragOver(index: number): void {
  if (dragIndex.value !== null && dragIndex.value !== index) {
    dragIndex.value = index;
  }
}

function onDragEnd(): void {
  dragId.value = null;
  dragIndex.value = null;
}

async function onDrop(): Promise<void> {
  const albumId = albumsStore.currentAlbumId;
  if (!isManualAlbum.value || !albumId || !dragId.value || dragIndex.value === null) return;
  const orderedIds = items.value.map((photo) => String(photo.id));
  const from = orderedIds.indexOf(dragId.value);
  if (from === -1 || from === dragIndex.value) {
    onDragEnd();
    return;
  }
  const [moved] = orderedIds.splice(from, 1);
  orderedIds.splice(dragIndex.value, 0, moved);
  const before = items.value.map((photo) => String(photo.id));
  if (JSON.stringify(before) === JSON.stringify(orderedIds)) {
    onDragEnd();
    return;
  }
  items.value = orderedIds.map((id) => itemsMap.value[id]).filter(Boolean);
  await albumsStore.reorderItems(albumId, orderedIds);
  onDragEnd();
}

function openNewAlbumDialog(): void {
  newAlbumName.value = '';
  newAlbumDialog.value = true;
}

async function createAlbum(): Promise<void> {
  const name = newAlbumName.value.trim();
  if (!name || creating.value) return;
  creating.value = true;
  try {
    const album = await albumsStore.createAlbum(name);
    if (album) {
      newAlbumDialog.value = false;
      openAlbum(album);
    }
  } finally {
    creating.value = false;
  }
}

function openRenameDialog(): void {
  if (!openedItem.value) return;
  renameName.value = openedItem.value.name;
  renameDialog.value = true;
}

async function renameCurrentAlbum(): Promise<void> {
  const name = renameName.value.trim();
  const item = openedItem.value;
  if (!name || !item?.album) return;
  await albumsStore.renameAlbum(item.album.id, name);
  item.name = name;
  renameDialog.value = false;
}

async function deleteCurrentAlbum(): Promise<void> {
  const item = openedItem.value;
  if (!item?.album) return;
  const name = item.name;
  await albumsStore.deleteAlbum(item.album.id);
  confirmDelete.value = false;
  closeAlbum();
  showMessage(t('albums.delete_confirm_title') + ': ' + name);
}

function computeColumns(): void {
  const width = window.innerWidth;
  if (width < 640) columns.value = 2;
  else if (width < 1024) columns.value = 3;
  else columns.value = 5;
}

onMounted(() => {
  computeColumns();
  window.addEventListener('resize', computeColumns);
  void albumsStore.loadSections();
});

onUnmounted(() => {
  window.removeEventListener('resize', computeColumns);
  if (searchTimer) clearTimeout(searchTimer);
});
</script>

<style scoped>
.albums-container {
  max-width: 980px;
  margin: 0 auto;
}

.album-grid {
  display: grid;
  gap: 16px;
}

.section-title {
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-size: 12px;
}

.manage-people-btn {
  display: inline-flex;
  align-items: center;
  font-size: 12px;
  font-weight: 600;
  color: rgb(var(--v-theme-primary));
  padding: 4px 10px;
  border-radius: var(--radius-md);
  cursor: pointer;
  user-select: none;
}

.manage-people-btn:hover {
  background: var(--color-bg-hover);
}

.album-card {
  cursor: pointer;
  transition: transform 0.2s ease;
}

.album-card:hover {
  transform: translateY(-2px);
}

.album-cover {
  position: relative;
  aspect-ratio: 1;
  border-radius: var(--radius-xl);
  overflow: hidden;
  background: var(--color-bg-field);
}

.album-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.album-cover-placeholder {
  width: 100%;
  height: 100%;
}

.album-count {
  position: absolute;
  bottom: 8px;
  left: 8px;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: var(--radius-pill);
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 11px;
  font-weight: 700;
  backdrop-filter: blur(4px);
}

.album-name {
  margin-top: 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.photo-row {
  display: grid;
  gap: 8px;
}

.drag-item {
  border-radius: var(--radius-md);
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.drag-item.drag-over {
  opacity: 0.5;
  transform: scale(0.96);
}

.empty-state-icon {
  width: 120px;
  height: 120px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--color-bg-zinc-100) 60%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
}

.bulk-toolbar-container {
  position: fixed;
  bottom: 88px;
  left: 0;
  right: 0;
  display: flex;
  justify-content: center;
  pointer-events: none;
  z-index: 1800;
}

.bulk-toolbar {
  pointer-events: auto;
}
</style>
