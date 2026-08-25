<template>
  <div class="albums-container px-4 py-6">
    <v-fade-transition>
      <div v-if="isManualAlbum && selectedIds.length > 0" class="bulk-toolbar-container">
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
              {{ $t('albums.selected', { count: selectedIds.length }) }}
            </div>
          </div>
          <v-spacer></v-spacer>
          <v-btn
            variant="flat"
            color="rgb(var(--v-theme-surface))"
            class="px-3 px-sm-6 rounded-xl text-none font-weight-bold"
            size="small"
            :aria-label="$t('albums.remove_from_album')"
            @click="bulkRemoveFromAlbum"
          >
            <v-icon size="16">mdi-minus-circle-outline</v-icon>
            <span class="d-none d-sm-inline ml-2">{{ $t('albums.remove_from_album') }}</span>
          </v-btn>
        </v-sheet>
      </div>
    </v-fade-transition>

    <template v-if="!currentSectionItem && !trashView">
      <div class="d-flex align-center px-2 mb-4">
        <div>
          <h1 class="text-h5 font-weight-bold text-high-emphasis letter-spacing-tight">
            {{ $t('albums.title') }}
          </h1>
          <p class="text-caption text-disabled">{{ $t('albums.desc') }}</p>
        </div>
        <v-spacer></v-spacer>
        <v-btn
          variant="flat"
          color="primary"
          class="new-album-btn px-3 px-sm-6"
          :aria-label="$t('albums.new_album')"
          @click="openNewAlbumDialog"
        >
          <v-icon size="18">mdi-plus</v-icon>
          <span class="d-none d-sm-inline ml-2">{{ $t('albums.new_album') }}</span>
        </v-btn>
      </div>

      <div class="d-flex ga-3 mb-6 px-2">
        <v-chip
          v-if="favoritesCount > 0"
          variant="tonal"
          color="primary"
          size="large"
          class="shortcut-chip"
          @click="openFavorites"
        >
          <v-icon start size="18">mdi-heart</v-icon>
          {{ $t('albums.section_favorites') }} ({{ favoritesCount }})
        </v-chip>
        <v-chip
          v-if="trashCount > 0"
          variant="tonal"
          color="error"
          size="large"
          class="shortcut-chip"
          @click="openTrash"
        >
          <v-icon start size="18">mdi-delete-outline</v-icon>
          {{ $t('albums.section_trash') }} ({{ trashCount }})
        </v-chip>
      </div>

      <PageLoading v-if="!hasAnyItems && albumsStore.sectionsLoading" class="py-12" />
      <template v-else-if="hasAnyItems">
        <div class="collections-grid mb-6">
          <div
            v-for="section in gridSections"
            :key="section.id"
            class="collection-tile"
            @click="openSectionItem(section.items[0])"
          >
            <div class="tile-preview">
              <template v-if="section.items.length >= 4">
                <div class="tile-mosaic">
                  <template v-for="(mosaicItem, i) in section.items.slice(0, 4)" :key="i">
                    <img
                      v-if="tileSrc(mosaicItem)"
                      :src="tileSrc(mosaicItem)"
                      :alt="mosaicItem.name"
                      loading="lazy"
                      class="mosaic-img"
                    />
                    <div v-else class="mosaic-placeholder d-flex align-center justify-center">
                      <v-icon size="24" color="rgba(var(--v-theme-on-surface), 0.25)">{{
                        tileIcon(section.id)
                      }}</v-icon>
                    </div>
                  </template>
                </div>
              </template>
              <template v-else-if="section.items.length > 0">
                <img
                  v-if="tileSrc(section.items[0])"
                  :src="tileSrc(section.items[0])"
                  :alt="section.items[0].name"
                  loading="lazy"
                  class="tile-cover-img"
                />
                <div v-else class="tile-cover-placeholder d-flex align-center justify-center">
                  <v-icon size="44" color="rgba(var(--v-theme-on-surface), 0.25)">{{
                    tileIcon(section.id)
                  }}</v-icon>
                </div>
              </template>
              <div v-else class="tile-cover-placeholder d-flex align-center justify-center">
                <v-icon size="44" color="rgba(var(--v-theme-on-surface), 0.25)">{{
                  tileIcon(section.id)
                }}</v-icon>
              </div>
            </div>
            <div class="tile-label d-flex align-center justify-space-between">
              <span class="text-subtitle-2 font-weight-bold text-high-emphasis">{{
                sectionTitle(section.id)
              }}</span>
              <span class="text-caption text-disabled"
                >{{ section.items.length }}
                {{
                  section.id === 'people' || section.id === 'places' || section.id === 'trips'
                    ? ''
                    : ''
                }}</span
              >
            </div>
          </div>
        </div>

        <div v-if="hasPeopleSection && peopleManageOpen" class="mb-8">
          <PeopleManagePanel
            :faces="unnamedFaces"
            :indexing-count="indexingCount"
            @start-indexing="startPeopleIndexing"
            @view-cluster="handleViewCluster"
            @prompt-name="promptName"
          />
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
        v-else
        class="empty-state-container d-flex flex-column align-center justify-center text-center"
      >
        <div
          class="create-album-card"
          role="button"
          tabindex="0"
          :aria-label="$t('albums.new_album')"
          @click="openNewAlbumDialog"
          @keydown.enter.prevent="openNewAlbumDialog"
          @keydown.space.prevent="openNewAlbumDialog"
        >
          <div class="create-album-icon mb-4">
            <v-icon size="28" color="rgb(var(--v-theme-primary))">mdi-plus</v-icon>
          </div>
          <h3 class="text-h6 font-weight-bold text-high-emphasis mb-1">
            {{ $t('albums.no_albums') }}
          </h3>
          <p class="text-body-2 text-medium-emphasis mb-5">{{ $t('albums.no_albums_hint') }}</p>
          <div class="benefit-list text-left">
            <div v-for="benefit in albumBenefits" :key="benefit.icon" class="benefit-item">
              <v-icon size="18" color="primary">{{ benefit.icon }}</v-icon>
              <span class="text-body-2 text-medium-emphasis">{{ benefit.text }}</span>
            </div>
          </div>
        </div>
      </div>
    </template>

    <template v-if="trashView">
      <div class="d-flex align-center px-2 mb-4">
        <v-btn icon variant="text" size="small" @click="closeTrash">
          <v-icon size="20">mdi-arrow-left</v-icon>
        </v-btn>
        <div class="ml-2">
          <h1 class="text-h6 font-weight-bold text-high-emphasis letter-spacing-tight">
            {{ $t('albums.section_trash') }}
          </h1>
          <p class="text-caption text-disabled">
            {{ trashPhotos.length }} {{ $t('albums.items_count', { count: trashPhotos.length }) }}
          </p>
        </div>
        <v-spacer></v-spacer>
        <v-btn
          variant="flat"
          color="error"
          class="px-3 px-sm-4"
          size="small"
          :aria-label="$t('albums.empty_trash')"
          @click="handleEmptyTrash"
        >
          <v-icon size="16">mdi-delete-sweep</v-icon>
          <span class="d-none d-sm-inline ml-2">{{ $t('albums.empty_trash') }}</span>
        </v-btn>
      </div>
      <PageLoading v-if="trashLoading" class="py-12" />
      <div
        v-else-if="trashPhotos.length > 0"
        class="photo-row"
        :style="{ gridTemplateColumns: `repeat(${columns}, 1fr)` }"
      >
        <div v-for="photo in trashPhotos" :key="photo.id" class="trash-photo-wrapper">
          <MediaCard :path="photo" :selected="false" :selection-mode="false" @click="() => {}" />
          <div class="trash-actions d-flex ga-1">
            <v-btn
              size="x-small"
              variant="flat"
              color="primary"
              @click="handleRestorePhoto(String(photo.id))"
            >
              <v-icon size="14">mdi-restore</v-icon>
            </v-btn>
            <v-btn
              size="x-small"
              variant="flat"
              color="error"
              @click="handleDeleteForever(String(photo.id))"
            >
              <v-icon size="14">mdi-delete</v-icon>
            </v-btn>
          </div>
        </div>
      </div>
      <div
        v-else
        class="empty-state-container d-flex flex-column align-center justify-center text-center py-12"
      >
        <v-icon size="80" color="rgba(var(--v-theme-on-surface), 0.25)"
          >mdi-delete-empty-outline</v-icon
        >
        <h3 class="text-h5 font-weight-bold text-high-emphasis mb-2 mt-4">
          {{ $t('albums.trash_empty') }}
        </h3>
      </div>
    </template>

    <template v-if="currentSectionItem && !trashView">
      <div class="d-flex align-center px-2 mb-2">
        <v-btn icon variant="text" size="small" :aria-label="$t('albums.back')" @click="closeAlbum">
          <v-icon size="20">mdi-arrow-left</v-icon>
        </v-btn>
        <div class="ml-2">
          <h1 class="text-h6 font-weight-bold text-high-emphasis letter-spacing-tight">
            {{ currentSectionItem?.name }}
          </h1>
          <p class="text-caption text-disabled">
            <template v-if="currentSectionItem">
              {{ kindLabel(currentSectionItem.kind) }}
              <span class="mx-1">·</span>
            </template>
            {{ $t('albums.items_count', { count: items.length }) }}
            <template v-if="isTrip && tripDateRange">
              <span class="mx-1">·</span>
              {{ tripDateRange }}
            </template>
          </p>
        </div>
        <v-spacer></v-spacer>
        <v-btn
          v-if="isTrip"
          icon
          variant="text"
          size="small"
          :aria-label="$t('albums.view_map')"
          @click="openTripMap"
        >
          <v-icon size="20">mdi-map-outline</v-icon>
        </v-btn>
        <v-menu v-if="currentSectionItem?.album || currentSectionItem?.kind === 'person'">
          <template v-slot:activator="{ props: menuProps }">
            <v-btn v-bind="menuProps" icon variant="text" size="small">
              <v-icon size="20">mdi-dots-vertical</v-icon>
            </v-btn>
          </template>
          <v-list density="compact">
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
              <v-list-item
                v-if="currentSectionItem?.album?.kind === 'manual'"
                @click="shareAlbum"
                prepend-icon="mdi-share-variant-outline"
              >
                <v-list-item-title>{{ $t('albums.share_album') }}</v-list-item-title>
              </v-list-item>
              <v-list-item @click="openRenameDialog" prepend-icon="mdi-pencil-outline">
                <v-list-item-title>{{ $t('albums.rename_album') }}</v-list-item-title>
              </v-list-item>
              <v-list-item
                @click="confirmDelete = true"
                prepend-icon="mdi-delete-outline"
                color="error"
              >
                <v-list-item-title>{{ $t('albums.delete_album') }}</v-list-item-title>
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
        class="text-caption text-disabled px-2 mb-2"
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
          <v-icon size="80" color="rgba(var(--v-theme-on-surface), 0.25)">mdi-image-outline</v-icon>
        </div>
        <h3 class="text-h5 font-weight-bold text-high-emphasis mb-2">
          {{ $t('albums.empty_album') }}
        </h3>
        <p class="text-body-1 text-medium-emphasis max-w-400 mx-auto mb-8">
          {{ $t('albums.empty_album_hint') }}
        </p>
      </div>

      <div
        v-else-if="searching && displayItems.length === 0 && !searchLoading"
        class="empty-state-container d-flex flex-column align-center justify-center text-center"
      >
        <div class="empty-state-icon mb-6">
          <v-icon size="80" color="rgba(var(--v-theme-on-surface), 0.25)"
            >mdi-text-search-variant</v-icon
          >
        </div>
        <h3 class="text-h5 font-weight-bold text-high-emphasis mb-2">
          {{ $t('albums.no_results_in_album', { query }) }}
        </h3>
      </div>

      <div class="loading-container py-8 d-flex justify-center">
        <v-fade-transition>
          <div v-if="loadingContents || searchLoading" class="d-flex align-center">
            <v-progress-circular
              indeterminate
              color="rgb(var(--v-theme-on-surface))"
              size="28"
              width="3"
            ></v-progress-circular>
          </div>
          <v-btn
            v-else-if="!searching && !allLoaded && items.length > 0"
            @click="loadMore"
            variant="outlined"
            size="small"
            class="px-4"
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
        <h3 class="text-h6 font-weight-bold text-high-emphasis mb-4">
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
        <h3 class="text-h6 font-weight-bold text-high-emphasis mb-4">
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
        <h3 class="text-h6 font-weight-bold text-high-emphasis mb-2">
          {{ $t('albums.delete_confirm_title') }}
        </h3>
        <p class="text-body-2 text-medium-emphasis mb-4">{{ $t('albums.delete_confirm') }}</p>
        <div class="d-flex justify-end ga-2">
          <v-btn variant="text" @click="confirmDelete = false">{{ $t('common.cancel') }}</v-btn>
          <v-btn variant="flat" color="error" @click="deleteCurrentAlbum">
            {{ $t('common.delete') }}
          </v-btn>
        </div>
      </v-card>
    </v-dialog>

    <v-dialog v-model="shareDialog" max-width="520">
      <v-card class="rounded-xl pa-6" color="surface">
        <h3 class="text-h6 font-weight-bold text-high-emphasis mb-2">
          {{ $t('albums.share_album') }}
        </h3>
        <p class="text-body-2 text-medium-emphasis mb-4">
          {{ $t('albums.share_album_desc') }}
        </p>
        <v-progress-linear v-if="shareLoading" indeterminate class="mb-4"></v-progress-linear>
        <v-text-field
          v-else
          :model-value="shareUrl"
          variant="outlined"
          density="comfortable"
          readonly
          hide-details
          class="mb-2"
        >
          <template v-slot:append-inner>
            <v-btn
              icon="mdi-content-copy"
              variant="text"
              size="small"
              :disabled="!shareUrl || shareUrl.startsWith('Error:')"
              @click="copyShareUrl"
            ></v-btn>
          </template>
        </v-text-field>
        <div class="d-flex justify-end mt-4">
          <v-btn variant="text" @click="shareDialog = false">{{ $t('common.close') }}</v-btn>
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
      <span class="text-body-2 text-high-emphasis">{{ snackbarText }}</span>
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { DynamicScroller, DynamicScrollerItem } from 'vue-virtual-scroller';
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css';
import { useI18n } from 'vue-i18n';
import { listen } from '@tauri-apps/api/event';
import MediaCard from '@/components/MediaCard.vue';
import MediaViewer from '@/components/MediaViewer.vue';
import PeopleManagePanel from '@/components/people/PeopleManagePanel.vue';
import NameDialog from '@/components/people/NameDialog.vue';
import ManageDialog from '@/components/people/ManageDialog.vue';
import ClusterDialog from '@/components/people/ClusterDialog.vue';
import PageLoading from '@/components/shared/PageLoading.vue';
import { useAlbumsStore } from '@/stores/albums';
import { useSearchStore } from '@/stores/search';
import { useUiStore } from '@/stores/ui';
import { useMapFilterStore } from '@/stores/mapFilter';
import { usePeople } from '@/composables/usePeople';
import { toggleFavorite, listFiles } from '@/services/tauri';
import { getFaceImageSrc } from '@/composables/useMediaUtils';
import { useMediaUrl } from '@/composables/useMediaUrl';
import type { Album, AlbumSectionItem } from '@/types/albums';
import type { MediaItem } from '@/types/media';
import type { ListFilesOptions } from '@/types/media';
import type { Person, UnnamedFace } from '@/types/person';

const { t } = useI18n();
const albumsStore = useAlbumsStore();
const searchStore = useSearchStore();
const uiStore = useUiStore();
const { thumbUrl: buildThumbUrl } = useMediaUrl();
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
const isTrip = computed(() => openedItem.value?.kind === 'trip');

const tripDateRange = computed(() => {
  const album = currentSectionItem.value?.album;
  if (!album?.rule) return null;
  try {
    const rule = JSON.parse(album.rule);
    const from = rule.date_from as string | undefined;
    const to = rule.date_to as string | undefined;
    if (!from && !to) return null;
    const fmtDate = (s: string) => {
      const d = s.slice(0, 10);
      const months = [
        'Jan',
        'Feb',
        'Mar',
        'Apr',
        'May',
        'Jun',
        'Jul',
        'Aug',
        'Sep',
        'Oct',
        'Nov',
        'Dec',
      ];
      const m = parseInt(d.slice(5, 7), 10) - 1;
      const day = parseInt(d.slice(8, 10), 10);
      const year = d.slice(0, 4);
      return `${months[m]} ${day}, ${year}`;
    };
    if (from && to) {
      const fromShort = from.slice(0, 10);
      const toShort = to.slice(0, 10);
      if (fromShort === toShort) return fmtDate(from);
      if (fromShort.slice(0, 7) === toShort.slice(0, 7)) {
        const m = parseInt(fromShort.slice(5, 7), 10) - 1;
        const months = [
          'Jan',
          'Feb',
          'Mar',
          'Apr',
          'May',
          'Jun',
          'Jul',
          'Aug',
          'Sep',
          'Oct',
          'Nov',
          'Dec',
        ];
        return `${months[m]} ${parseInt(fromShort.slice(8, 10), 10)} – ${parseInt(toShort.slice(8, 10), 10)}, ${fromShort.slice(0, 4)}`;
      }
      return `${fmtDate(from)} – ${fmtDate(to)}`;
    }
    return from ? fmtDate(from) : fmtDate(to!);
  } catch {
    return null;
  }
});

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
const shareDialog = ref(false);
const shareUrl = ref('');
const shareLoading = ref(false);
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

const albumBenefits = computed(() => [
  { icon: 'mdi-image-multiple-outline', text: t('albums.benefit_organize') },
  { icon: 'mdi-auto-fix', text: t('albums.benefit_smart') },
  { icon: 'mdi-shield-lock-outline', text: t('albums.benefit_private') },
]);

// Grid sections for the 2x2 tile layout (favorites, trash, people, places, trips, albums, documents)
const gridSections = computed(() => {
  return sections.value.filter((s) =>
    ['favorites', 'trash', 'people', 'places', 'trips', 'albums', 'documents'].includes(s.id),
  );
});

// Trash view state
const trashView = ref(false);
const trashPhotos = ref<MediaItem[]>([]);
const trashLoading = ref(false);
const trashCount = computed(() => {
  const trashSection = sections.value.find((s) => s.id === 'trash');
  return trashSection?.items[0]?.count ?? 0;
});
const favoritesCount = computed(() => {
  const favSection = sections.value.find((s) => s.id === 'favorites');
  return favSection?.items.length ?? 0;
});

async function openTrash(): Promise<void> {
  trashView.value = true;
  trashLoading.value = true;
  try {
    const { listTrash } = await import('@/services/tauri');
    trashPhotos.value = await listTrash(200);
  } catch (e) {
    console.error('[Collections] Failed to load trash:', e);
  } finally {
    trashLoading.value = false;
  }
}

function closeTrash(): void {
  trashView.value = false;
  trashPhotos.value = [];
}

async function handleRestorePhoto(id: string): Promise<void> {
  const { restorePhoto } = await import('@/services/tauri');
  await restorePhoto(id);
  trashPhotos.value = trashPhotos.value.filter((p) => String(p.id) !== id);
  await albumsStore.loadSections();
}

async function handleDeleteForever(id: string): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('delete_photo_permanently', { id });
  trashPhotos.value = trashPhotos.value.filter((p) => String(p.id) !== id);
  await albumsStore.loadSections();
}

async function handleEmptyTrash(): Promise<void> {
  const { emptyTrash } = await import('@/services/tauri');
  await emptyTrash();
  trashPhotos.value = [];
  await albumsStore.loadSections();
  closeTrash();
}

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
  if (item.cover_location) return buildThumbUrl(item.cover_location) ?? '';
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
    case 'favorites':
      return 'mdi-heart';
    case 'trash':
      return 'mdi-delete-outline';
    case 'places':
      return 'mdi-map-marker';
    case 'albums':
      return 'mdi-image-multiple-outline';
    case 'documents':
      return 'mdi-file-document-outline';
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
    cover_location: null,
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

function openTripMap(): void {
  const album = openedItem.value?.album;
  if (!album?.rule) return;
  try {
    const rule = JSON.parse(album.rule);
    uiStore.setPage('location');
    const mapStore = useMapFilterStore();
    mapStore.setDateRange(rule.date_from ?? null, rule.date_to ?? null);
  } catch {
    uiStore.setPage('location');
  }
}

function openFavorites(): void {
  const favSection = sections.value.find((s) => s.id === 'favorites');
  if (favSection && favSection.items.length > 0) {
    openSectionItem(favSection.items[0]);
  }
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
  if (item.kind === 'favorites') {
    return { favoritesOnly: true };
  }
  if (item.kind === 'location' && item.id.startsWith('location:')) {
    return { location: item.id.slice('location:'.length) };
  }
  if (item.kind === 'document') {
    return { papers: true };
  }
  return { albumId: item.id };
}

async function loadContents(): Promise<void> {
  const item = openedItem.value;
  if (!item) return;
  loadingContents.value = true;
  try {
    let photos: MediaItem[];
    if (
      item.kind === 'person' ||
      item.kind === 'favorites' ||
      item.kind === 'location' ||
      item.kind === 'document'
    ) {
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

async function shareAlbum(): Promise<void> {
  const album = currentSectionItem.value?.album;
  if (!album) return;
  shareLoading.value = true;
  shareDialog.value = true;
  shareUrl.value = '';
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const url = await invoke<string>('generate_album_share_url', { albumId: album.id });
    shareUrl.value = url;
  } catch (e) {
    shareUrl.value = `Error: ${String(e)}`;
  } finally {
    shareLoading.value = false;
  }
}

function copyShareUrl(): void {
  if (shareUrl.value && !shareUrl.value.startsWith('Error:')) {
    navigator.clipboard.writeText(shareUrl.value);
    snackbarText.value = $t('albums.share_link_copied');
    snackbar.value = true;
    shareDialog.value = false;
  }
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

let unlistenRefreshed: (() => void) | null = null;

onMounted(async () => {
  computeColumns();
  window.addEventListener('resize', computeColumns);
  void albumsStore.loadSections();
  unlistenRefreshed = await listen('photos-refreshed', () => {
    void albumsStore.loadSections();
  });
});

onUnmounted(() => {
  window.removeEventListener('resize', computeColumns);
  if (searchTimer) clearTimeout(searchTimer);
  unlistenRefreshed?.();
});
</script>

<style scoped>
.albums-container {
  max-width: 980px;
  margin: 0 auto;
}

.collections-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 16px;
}

.collection-tile {
  cursor: pointer;
  border-radius: 20px;
  overflow: hidden;
  background: rgb(var(--v-theme-surface-light));
  transition:
    transform 0.2s ease,
    box-shadow 0.2s ease;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.08);
}

.collection-tile:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.tile-preview {
  aspect-ratio: 1.2;
  overflow: hidden;
}

.tile-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.tile-cover-placeholder {
  width: 100%;
  height: 100%;
}

.tile-mosaic {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2px;
  height: 100%;
}

.mosaic-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.mosaic-placeholder {
  width: 100%;
  height: 100%;
  background: rgb(var(--v-theme-surface-light));
}

.tile-label {
  padding: 12px 16px;
}

.shortcut-chip {
  cursor: pointer;
  font-weight: 600;
}

.section-title {
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-size: 12px;
}

.photo-row {
  display: grid;
  gap: 8px;
}

.trash-photo-wrapper {
  position: relative;
}

.trash-actions {
  position: absolute;
  bottom: 8px;
  right: 8px;
  z-index: 10;
}

.drag-item {
  border-radius: 8px;
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
  border-radius: 9999px;
  background: rgb(var(--v-theme-surface-light));
  display: flex;
  align-items: center;
  justify-content: center;
}

.create-album-card {
  width: 100%;
  max-width: 340px;
  padding: 32px 24px;
  border-radius: 20px;
  background: rgb(var(--v-theme-surface-light));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.08);
  cursor: pointer;
  transition:
    transform 0.2s cubic-bezier(0.4, 0, 0.2, 1),
    box-shadow 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.create-album-card:hover {
  transform: translateY(-2px) scale(1.01);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
}

.create-album-card:active {
  transform: scale(0.98);
}

.create-album-card:focus-visible {
  outline: 2px solid rgb(var(--v-theme-primary));
  outline-offset: 2px;
}

.create-album-icon {
  width: 64px;
  height: 64px;
  margin-left: auto;
  margin-right: auto;
  border-radius: 9999px;
  background: rgba(var(--v-theme-primary), 0.12);
  display: flex;
  align-items: center;
  justify-content: center;
}

.benefit-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.benefit-item {
  display: flex;
  align-items: center;
  gap: 10px;
}

@media (max-width: 600px) {
  .empty-state-icon {
    width: 88px;
    height: 88px;
  }

  .empty-state-icon :deep(.v-icon),
  .empty-state-container > .v-icon {
    font-size: 48px !important;
  }

  .create-album-card {
    padding: 24px 18px;
    max-width: 100%;
  }
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
