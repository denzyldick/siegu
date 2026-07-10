<template>
  <div class="photos-container px-4 py-6">
    <!-- Bulk Actions Toolbar -->
    <v-fade-transition>
      <div v-if="selectedIds.length > 0" class="bulk-toolbar-container">
        <v-sheet class="bulk-toolbar d-flex align-center px-6 py-3 rounded-pill shadow-xl" color="#18181b">
          <v-btn icon="mdi-close" variant="text" density="comfortable" color="white" @click="clearSelection"></v-btn>
          <div class="ml-4">
            <div class="text-subtitle-2 font-weight-bold text-white">{{ $t('photos.items_selected', { count: selectedIds.length }) }}</div>
          </div>
          <v-spacer></v-spacer>
          <div class="d-flex ga-2">
            <v-btn
              variant="flat"
              class="siegu-btn-modern px-6"
              size="small"
              @click="bulkFavorite"
            >
              <v-icon size="16" class="mr-2">mdi-heart</v-icon>
              <span>{{ $t('photos.favorite') }}</span>
            </v-btn>
            <v-btn
              variant="flat"
              color="rgba(255,255,255,0.1)"
              class="text-white px-6 rounded-xl text-none font-weight-bold"
              size="small"
              @click="bulkRemove"
            >
              {{ $t('photos.remove') }}
            </v-btn>
          </div>
        </v-sheet>
      </div>
    </v-fade-transition>

    <!-- Virtual Scroller Grouped View -->
    <DynamicScroller
      v-if="groups.length > 0"
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
            <h2 class="text-h5 font-weight-bold text-zinc-primary letter-spacing-tight">{{ item.name }}</h2>
            <v-spacer></v-spacer>
            <span class="text-caption text-zinc-muted font-weight-medium bg-zinc-100 px-3 py-1 rounded-pill border-subtle">
              {{ $t('photos.items_count', { count: item.count }) }}
            </span>
          </div>
        </div>
        <div v-else class="photo-row" :style="{ gridTemplateColumns: `repeat(${columns}, 1fr)` }">
          <Image
            v-for="photo in item.photos"
            v-bind:key="photo.id"
            :path="photo"
            :selected="selectedIds.includes(photo.id)"
            :selection-mode="selectedIds.length > 0"
            @click="openViewerByPhoto(photo)"
            @select="toggleSelection"
            @toggle-favorite="handleToggleFavorite"
          />
        </div>
      </DynamicScrollerItem>
    </DynamicScroller>

    <!-- Empty States -->
    <div v-else-if="!loading" class="empty-state-container d-flex flex-column align-center justify-center text-center">
      <div class="empty-state-icon mb-6">
        <template v-if="searchQuery">
          <v-icon size="80" color="#d4d4d8">mdi-text-search-variant</v-icon>
        </template>
        <template v-else-if="filters.favoritesOnly">
          <v-icon size="80" color="#fee2e2">mdi-heart-multiple</v-icon>
        </template>
        <template v-else>
          <v-icon size="80" color="#f4f4f5">mdi-image-multiple-outline</v-icon>
        </template>
      </div>

      <h3 class="text-h5 font-weight-bold text-zinc-primary mb-2">
        {{ searchQuery ? $t('photos.no_results') : (filters.favoritesOnly ? $t('photos.no_favorites') : $t('photos.your_library_empty')) }}
      </h3>
      <p class="text-body-1 text-zinc-secondary max-w-400 mx-auto mb-8">
        {{ searchQuery ? $t('photos.no_results_for', { query: searchQuery }) : (filters.favoritesOnly ? $t('photos.tap_heart_hint') : $t('photos.add_folder_hint')) }}
      </p>

      <v-btn v-if="searchQuery" variant="flat" class="siegu-btn-modern px-8 py-6" @click="$emit('clear-search')">
        {{ $t('photos.clear_search') }}
      </v-btn>
    </div>

    <!-- Loading State & Infinite Scroll -->
    <div id="scroll-sentinel" class="scroll-sentinel"></div>

    <div class="loading-container py-12 d-flex justify-center">
      <v-fade-transition>
        <div v-if="loading" class="d-flex flex-column align-center">
          <v-progress-circular indeterminate color="#18181b" size="32" width="3"></v-progress-circular>
          <span class="mt-4 text-caption text-zinc-muted font-weight-medium tracking-widest text-uppercase">{{ $t('photos.loading_memories') }}</span>
        </div>
        <v-btn
          v-else-if="!allLoaded && groups.length > 0"
          @click="list_files"
          variant="flat"
          class="siegu-btn-outline px-10 py-6"
        >
          {{ $t('photos.load_more') }}
        </v-btn>
      </v-fade-transition>
    </div>

    <PhotoViewer
      v-model="viewerOpen"
      :photos="images"
      v-model:index="currentPhotoIndex"
      @navigate-to-person="$emit('search-person', $event)"
      @update:photo="handlePhotoUpdated"
    />
  </div>
</template>

<script>
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DynamicScroller, DynamicScrollerItem } from "vue-virtual-scroller";
import "vue-virtual-scroller/dist/vue-virtual-scroller.css";
import Image from "./Image.vue";
import PhotoViewer from "./PhotoViewer.vue";

export default {
  name: "Photos",
  components: { DynamicScroller, DynamicScrollerItem, Image, PhotoViewer },
  data: () => ({
    loading: false,
    allLoaded: false,
    paging: {
      offset: 0,
      limit: 50,
    },
    images: [],
    imagesMap: {},
    groups: [],
    groupsMap: {},
    selectedIds: [],
    viewerOpen: false,
    currentPhotoIndex: 0,
    observer: null,
    unlistenDiscovered: null,
    unlistenAnalysisResult: null,
    reloadTimer: null,
    columns: 5,
  }),
  props: {
    searchQuery: {
      type: String,
      default: "",
    },
    isPersonFilter: {
      type: Boolean,
      default: false,
    },
    filters: {
      type: Object,
      default: () => ({
        favoritesOnly: false,
        videosOnly: false,
        dateRange: 'all',
        folder: null,
      })
    }
  },
  computed: {
    virtualItems() {
      const cols = this.columns;
      const items = [];
      for (const group of this.groups) {
        items.push({
          type: 'header',
          key: `h-${group.name}`,
          name: group.name,
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
    },
  },
  async created() {
    this.list_files();

    this.unlistenDiscovered = await listen("photos-discovered", (event) => {
      console.log("photos-discovered", event.payload?.length ?? 0, "photos");
      if (Array.isArray(event.payload)) {
        this.updateGroups(event.payload);
      }
    });

    this.unlistenAnalysisResult = await listen("photo-analysis-result", async (event) => {
      const { id } = event.payload;
      if (!id) return;
      try {
        const raw = await invoke("get_photo_by_id", { id });
        if (raw && raw !== "null") {
          const updated = JSON.parse(raw);
          const existing = this.imagesMap[id];
          if (existing) {
            updated._groupKey = existing._groupKey;
            updated._sortKey = existing._sortKey;
            this.imagesMap[id] = updated;
            const idx = this.images.findIndex(p => p.id === id);
            if (idx !== -1) {
              this.images[idx] = updated;
            }
            for (const g of this.groups) {
              const gi = g.images.findIndex(p => p.id === id);
              if (gi !== -1) {
                g.images[gi] = updated;
              }
            }
          } else {
            this.updateGroups([updated]);
          }
        }
      } catch (e) {
        console.warn("Failed to fetch updated photo after analysis:", e);
      }
    });
  },
  mounted() {
    this.updateColumns();
    window.addEventListener('resize', this.updateColumns);
    if (!this.isPersonFilter) {
      this.setupInfiniteScroll();
    }
  },
  beforeUnmount() {
    window.removeEventListener('resize', this.updateColumns);
    if (this.observer) this.observer.disconnect();
    if (this.unlistenDiscovered) this.unlistenDiscovered();
    if (this.unlistenAnalysisResult) this.unlistenAnalysisResult();
    if (this.reloadTimer) clearTimeout(this.reloadTimer);
  },
  methods: {
    updateColumns() {
      const width = window.innerWidth;
      if (width < 640) this.columns = 2;
      else if (width < 1024) this.columns = 3;
      else this.columns = 5;
    },
    updateGroups(newImages) {
        const locale = localStorage.getItem('siegu_language') || 'en';
        const affectedGroups = new Set();
        
        newImages.forEach(image => {
            if (this.imagesMap[image.id]) return;
            
            this.imagesMap[image.id] = image;
            this.images.push(image);

            if (!image._groupKey) {
                if (image.created) {
                    const datePart = image.created.split(' ')[0];
                    const dateParts = datePart.includes(':') ? datePart.split(':') : datePart.split('-');
                    if (dateParts.length >= 2) {
                        const year = dateParts[0];
                        const monthIdx = parseInt(dateParts[1]) - 1;
                        if (monthIdx >= 0 && monthIdx < 12) {
                            const monthName = new Date(parseInt(year), monthIdx).toLocaleString(locale, { month: 'long' });
                            image._groupKey = `${monthName} ${year}`;
                            image._sortKey = `${year}${dateParts[1].padStart(2, '0')}`;
                        }
                    }
                }
                if (!image._groupKey) {
                    image._groupKey = this.$t('photos.recent');
                    image._sortKey = "999999";
                }
            }

            let group = this.groupsMap[image._groupKey];
            if (!group) {
                group = { name: image._groupKey, sortKey: image._sortKey, images: [] };
                this.groupsMap[image._groupKey] = group;
                this.groups.push(group);
                this.groups.sort((a, b) => b.sortKey.localeCompare(a.sortKey));
            }
            group.images.push(image);
            affectedGroups.add(group);
        });

        affectedGroups.forEach(group => {
            group.images.sort((a, b) => (b.created || '').localeCompare(a.created || ''));
        });
    },
    handlePhotoUpdated(updatedPhoto) {
      const existing = this.imagesMap[updatedPhoto.id];
      if (existing) {
        Object.assign(existing, updatedPhoto);
      } else {
        this.updateGroups([updatedPhoto]);
      }
    },
    toggleSelection(id) {
      const index = this.selectedIds.indexOf(id);
      if (index === -1) {
        this.selectedIds.push(id);
      } else {
        this.selectedIds.splice(index, 1);
      }
    },
    clearSelection() {
      this.selectedIds = [];
    },
    async bulkFavorite() {
      const ids = [...this.selectedIds];
      for (const id of ids) {
        await this.handleToggleFavorite(id);
      }
      this.clearSelection();
    },
    async bulkRemove() {
      this.clearSelection();
    },
    setupInfiniteScroll() {
      this.observer = new IntersectionObserver((entries) => {
        if (entries[0].isIntersecting && !this.loading && !this.allLoaded) {
          this.list_files();
        }
      }, {
        threshold: 0.01,
        rootMargin: '600px'
      });

      const sentinel = document.getElementById('scroll-sentinel');
      if (sentinel) this.observer.observe(sentinel);
    },
    list_files: async function () {
      if (this.loading) return;
      this.loading = true;

      try {
        let response;
        if (this.isPersonFilter && this.searchQuery) {
          response = await invoke("get_person_photos", { personId: this.searchQuery });
          this.allLoaded = true;
        } else {
          response = await invoke("list_files", {
            offset: this.paging.offset,
            limit: this.paging.limit,
            query: this.searchQuery ?? "",
            scan: false,
            favoritesOnly: this.filters.favoritesOnly,
            videosOnly: this.filters.videosOnly,
          });
        }

        const new_images = JSON.parse(response);

        if (this.paging.offset === 0) {
          this.imagesMap = {};
          this.groupsMap = {};
          this.groups = [];
          this.images = [];
          this.updateGroups(new_images);
        } else {
          this.updateGroups(new_images);
        }

        if (!this.isPersonFilter) {
          if (new_images.length < this.paging.limit) {
            this.allLoaded = true;
          } else {
            this.paging.offset += this.paging.limit;
          }
        }
      } catch (err) {
        console.error("Failed to list files:", err);
      } finally {
        this.loading = false;
      }
    },
    scheduleReload() {
      if (this.reloadTimer) clearTimeout(this.reloadTimer);
      this.reloadTimer = setTimeout(() => {
        this.paging.offset = 0;
        this.allLoaded = false;
        this.list_files();
      }, 200);
    },
    async handleToggleFavorite(id) {
      try {
        const isNowFavorite = await invoke("toggle_favorite", { id: id });
        const photo = this.imagesMap[id];
        if (photo) {
          photo.favorite = isNowFavorite;
          if (this.filters.favoritesOnly && !isNowFavorite) {
            this.images = this.images.filter((p) => p.id !== id);
            delete this.imagesMap[id];
            const group = this.groupsMap[photo._groupKey];
            if (group) {
                group.images = group.images.filter(p => p.id !== id);
            }
          }
        }
      } catch (err) {
        console.error("Failed to toggle favorite:", err);
      }
    },
    openViewer(index) {
      this.currentPhotoIndex = index;
      this.viewerOpen = true;
    },
    openViewerByPhoto(photo) {
      const index = this.images.findIndex(p => p.id === photo.id);
      if (index !== -1) this.openViewer(index);
    },
  },
  watch: {
    searchQuery() {
      this.scheduleReload();
    },
    filters: {
      deep: true,
      handler() {
        this.scheduleReload();
      }
    }
  },
};
</script>

<style scoped>
.photos-container {
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
  background: rgba(250, 250, 250, 0.8);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
}

.letter-spacing-tight {
  letter-spacing: -0.02em;
}

.bg-zinc-100 {
  background-color: var(--color-bg-zinc-100);
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
  box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
}

.siegu-btn-modern {
  background: #000000;
  color: #ffffff;
  border-radius: 12px;
  text-transform: none;
  font-weight: 700;
  box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
}

.siegu-btn-outline {
  background: var(--color-bg-surface);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border-default);
  border-radius: 12px;
  text-transform: none;
  font-weight: 600;
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
  from { opacity: 0; transform: translateY(20px); }
  to { opacity: 1; transform: translateY(0); }
}

.scroll-sentinel {
  height: 20px;
}
</style>
