<template>
  <v-dialog v-model="visible" fullscreen transition="dialog-bottom-transition">
    <v-card rounded="0" color="background" class="fill-height" style="overflow: hidden;">
      <v-layout class="fill-height">
        <!-- Main Viewer Area -->
        <v-main class="fill-height position-relative d-flex flex-column align-center justify-center p-0" style="background-color: rgb(var(--v-theme-background));">

          <!-- Top Controls -->
          <v-btn icon="mdi-close" variant="text" color="#18181b" class="viewer-nav-btn top-left" @click="close"></v-btn>
          <v-btn
            v-if="!isVideo && !showInfo"
            icon="mdi-information-outline"
            variant="text"
            color="#71717a"
            class="viewer-nav-btn top-right"
            @click="showInfo = !showInfo"
          ></v-btn>

          <!-- Interaction Layer -->
          <div class="touch-overlay"
               v-touch="{
                 left: () => next(),
                 right: () => prev(),
                 down: () => close()
               }">
          </div>

          <!-- Content Layer -->
          <div class="viewer-content-container">
            <v-btn v-if="!isMobile" icon="mdi-chevron-left" variant="text" color="#18181b" size="x-large" @click="prev" class="side-nav-btn left"></v-btn>

            <div class="media-wrapper">
              <img v-if="currentPhoto && !isVideo" :src="currentPhotoSrc" class="viewer-image" />
              <video
                v-if="currentPhoto && isVideo"
                :src="videoUrl"
                class="viewer-image"
                controls
                autoplay
                style="z-index: 10; position: relative;"
              ></video>
            </div>

            <v-btn v-if="!isMobile" icon="mdi-chevron-right" variant="text" color="#18181b" size="x-large" @click="next" class="side-nav-btn right"></v-btn>
          </div>

          <!-- Bottom Thumbnail Rail -->
          <div class="thumbnail-rail-container">
            <div class="thumbnail-rail" ref="thumbnailRail">
              <RailItem
                v-for="(photo, i) in photos"
                :key="photo.id"
                :photo="photo"
                :active="i === index"
                @click="$emit('update:index', i)"
              />
            </div>
          </div>

        </v-main>

        <!-- Info Drawer -->
        <v-navigation-drawer
          v-if="showInfo"
          v-model="showInfo"
          location="right"
          width="350"
          color="surface"
          class="border-s border-subtle info-drawer"
          temporary
        >
        <v-toolbar color="transparent" density="compact">
          <v-toolbar-title class="text-zinc-primary text-subtitle-1 font-weight-bold">{{ $t('photo_viewer.metadata') }}</v-toolbar-title>
        </v-toolbar>

        <v-divider class="opacity-5"></v-divider>

        <v-list class="bg-transparent px-4">
          <div class="mb-4" v-if="currentPhoto?.indexed < 2">
             <v-btn
               block
               variant="flat"
               color="black"
               prepend-icon="mdi-auto-fix"
               :loading="isAnalyzing"
               @click="analyzePhoto"
               class="text-none"
             >
                {{ $t('photo_viewer.analyze') }}
              </v-btn>
               <div v-if="isAnalyzing" class="text-caption text-zinc-muted mt-2 text-center">
                 <span class="analyzing-dots">{{ $t('photo_viewer.analyzing') }}</span>
              </div>
              <div v-else-if="globalEta" class="text-caption text-zinc-muted mt-2 text-center">
                 {{ $t('photo_viewer.library_indexing', { time: formatEta(globalEta) }) }}
              </div>
          </div>

          <v-divider class="opacity-5 mb-4" v-if="modelChips.length > 0"></v-divider>

          <div class="mb-6" v-if="modelChips.length > 0">
            <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">{{ $t('photo_viewer.run_model') }}</div>
            <div v-if="isAnalyzingModel" class="d-flex align-center mb-3">
              <v-progress-circular indeterminate size="16" width="2" color="black" class="mr-2"></v-progress-circular>
              <span class="text-body-2 text-zinc-primary font-weight-bold">
                {{ $t('photo_viewer.running_model', { model: $t('models.' + isAnalyzingModel + '.title') }) }}
                <span class="text-caption text-zinc-muted ml-1">({{ formatElapsed(runStartTime, runTimerTick) }})</span>
              </span>
            </div>
            <div class="d-flex flex-wrap ga-2">
              <v-chip
                v-for="m in modelChips"
                :key="m.id"
                :variant="m.done ? 'tonal' : 'flat'"
                :color="m.done ? 'success' : 'black'"
                size="small"
                :prepend-icon="m.done ? 'mdi-check-circle-outline' : 'mdi-play-circle-outline'"
                :disabled="m.done || (isAnalyzingModel !== null && isAnalyzingModel !== m.id)"
                :loading="isAnalyzingModel === m.id"
                @click="runSingleModel(m.id)"
                class="font-weight-bold"
              >
                {{ $t('models.' + m.id + '.title') }}
              </v-chip>
            </div>
          </div>

          <div class="mb-6 pt-4">
            <div class="text-caption text-zinc-muted mb-1 text-uppercase tracking-widest">{{ $t('photo_viewer.file_details') }}</div>
            <div class="d-flex align-start mb-2">
              <v-icon size="small" color="#71717a" class="mr-2 mt-1">mdi-file-document-outline</v-icon>
              <div class="text-body-2 text-zinc-secondary word-break-all">
                {{ currentPhoto?.location }}
              </div>
            </div>
          </div>

          <v-divider class="opacity-5 mb-4"></v-divider>

          <!-- Caption Section -->
          <div class="mb-6" v-if="currentPhoto?.caption">
            <div class="text-caption text-zinc-muted mb-1 text-uppercase tracking-widest">{{ $t('photo_viewer.ai_caption') }}</div>
            <div class="text-body-2 text-zinc-primary font-italic">
              "{{ currentPhoto.caption }}"
            </div>
          </div>

          <v-divider class="opacity-5 mb-4" v-if="currentPhoto?.caption"></v-divider>

          <div class="mb-6" v-if="hasExif">
            <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">{{ $t('photo_viewer.camera_settings') }}</div>

            <div class="d-flex align-center mb-4" v-if="exifData.make || exifData.model">
              <v-icon size="small" color="#71717a" class="mr-2">mdi-camera</v-icon>
              <span class="text-body-2 text-zinc-secondary">{{ exifData.make }} {{ exifData.model }}</span>
            </div>

            <v-row dense>
              <v-col cols="6" v-if="exifData.date" class="mb-3">
                <div class="text-caption text-zinc-muted">{{ $t('photo_viewer.date_taken') }}</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.date }}</div>
              </v-col>
              <v-col cols="6" v-if="exifData.dimensions" class="mb-3">
                <div class="text-caption text-zinc-muted">{{ $t('photo_viewer.resolution') }}</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.dimensions }}</div>
              </v-col>
              <v-col cols="6" v-if="exifData.iso" class="mb-3">
                <div class="text-caption text-zinc-muted">{{ $t('photo_viewer.iso') }}</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.iso }}</div>
              </v-col>
              <v-col cols="6" v-if="exifData.shutter" class="mb-3">
                <div class="text-caption text-zinc-muted">{{ $t('photo_viewer.shutter') }}</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.shutter }}</div>
              </v-col>
              <v-col cols="6" v-if="exifData.aperture" class="mb-3">
                <div class="text-caption text-zinc-muted">{{ $t('photo_viewer.aperture') }}</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.aperture }}</div>
              </v-col>
            </v-row>
          </div>

          <v-divider class="opacity-5 mb-4"></v-divider>

          <div class="mb-6">
            <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">{{ $t('photo_viewer.people_in_photo') }}</div>
            <div v-if="detectedFaces.length === 0" class="text-body-2 text-zinc-muted font-italic">
              {{ $t('photo_viewer.no_faces') }}
            </div>
            <div class="d-flex flex-wrap ga-3">
              <div 
                v-for="face in uniquePeople" 
                :key="face.face_id" 
                class="d-flex flex-column align-center cursor-pointer"
                @click="goToPerson(face)"
                style="width: 70px;"
              >
                <v-avatar size="56" class="border-subtle mb-1">
                  <v-img :src="face.encoded" cover></v-img>
                </v-avatar>
                <div class="text-caption text-zinc-primary text-truncate text-center w-100 font-weight-bold">
                  {{ face.person_name || $t('photo_viewer.unnamed') }}
                </div>
              </div>
            </div>
          </div>

          <v-divider class="opacity-5 mb-4"></v-divider>

          <div class="mb-6">
            <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">{{ $t('photo_viewer.ai_insights') }}</div>

            <div v-if="aiTags.length === 0" class="text-body-2 text-zinc-muted font-italic">
              {{ $t('photo_viewer.no_insights') }}
            </div>

            <div v-for="tag in aiTags" :key="tag.name" class="mb-4">
              <div class="d-flex align-center justify-space-between w-100">
                <span class="text-body-2 text-zinc-secondary text-capitalize">{{ tag.name }}</span>
                <span class="text-caption text-zinc-muted">{{ tag.percent }}%</span>
              </div>
              <v-progress-linear
                :model-value="tag.percent"
                color="#18181b"
                height="2"
                rounded
                class="mt-1 opacity-10"
              ></v-progress-linear>
            </div>
          </div>

        </v-list>
      </v-navigation-drawer>
      </v-layout>
      <v-snackbar v-model="snackbar.show" :timeout="6000" location="bottom" color="black">
        <div class="d-flex align-center">
          <v-icon size="small" class="mr-3" :color="snackbar.error ? 'error' : 'white'">{{ snackbar.error ? 'mdi-alert-circle' : 'mdi-check-circle' }}</v-icon>
          <span class="text-body-2">{{ snackbar.text }}</span>
        </div>
      </v-snackbar>
    </v-card>
  </v-dialog>
</template>

<script>
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import RailItem from './RailItem.vue';

export default {
  name: "PhotoViewer",
  components: { RailItem },
  props: {
    modelValue: Boolean,
    photos: {
      type: Array,
      default: () => []
    },
    index: {
      type: Number,
      default: 0
    }
  },
  emits: ['update:modelValue', 'update:index', 'update:photo'],
  data: () => ({
    showInfo: false,
    os: '',
    mediaPort: null,
    detectedFaces: [],
    isAnalyzing: false,
    isAnalyzingModel: null,
    runStartTime: 0,
    runTimerTick: 0,
    runTimer: null,
    globalEta: 0,
    unlistenEta: null,
    unlistenResult: null,
    snackbar: { show: false, text: '', error: false },
    downloadedModels: [],
    isAnalyzingModel: null,
    modelInfo: [
      { id: 'clip' },
      { id: 'ultraface' },
      { id: 'ocr' },
      { id: 'nsfw' },
      { id: 'aesthetics' },
      { id: 'yolo' },
      { id: 'blip' },
      { id: 'arcface' },
      { id: 'midas' },
    ],
  }),
  computed: {
    isMobile() {
      return this.os === 'android' || this.os === 'ios';
    },
    visible: {
      get() { return this.modelValue; },
      set(val) { this.$emit('update:modelValue', val); }
    },
    currentPhoto() {
      if (!this.photos || this.photos.length === 0) return null;
      return this.photos[this.index];
    },
    isVideo() {
      return this.isVideoPhoto(this.currentPhoto);
    },
    videoUrl() {
      if (!this.currentPhoto || !this.isVideo || !this.mediaPort) return '';
      let path = this.currentPhoto.location.replace(/\\/g, '/');
      if (path.match(/^[a-zA-Z]:\//)) {
          path = path.substring(3);
      } else if (path.startsWith('/')) {
          path = path.substring(1);
      }
      const encoded = path.split('/').map(encodeURIComponent).join('/');
      return `http://127.0.0.1:${this.mediaPort}/media/${encoded}`;
    },
    currentPhotoSrc() {
      if (!this.currentPhoto || this.isVideo) return '';
      const ext = this.currentPhoto.location.split('.').pop().toLowerCase();
      if (['heic', 'heif'].includes(ext)) {
        return this.currentPhoto.encoded || convertFileSrc(this.currentPhoto.location);
      }
      return convertFileSrc(this.currentPhoto.location);
    },
    exifData() {
      if (!this.currentPhoto || !this.currentPhoto.properties) return {};
      const props = this.currentPhoto.properties;
      let dimensions = null;
      if (props.PixelXDimension && props.PixelYDimension) {
        dimensions = `${props.PixelXDimension} x ${props.PixelYDimension}`;
      } else if (props.ImageWidth && props.ImageLength) {
        dimensions = `${props.ImageWidth} x ${props.ImageLength}`;
      }
      return {
        make: props.Make,
        model: props.Model,
        date: props.DateTimeOriginal || props.DateTime,
        dimensions,
        iso: props.PhotographicSensitivity || props.ISOSpeedRatings,
        shutter: props.ExposureTime,
        aperture: props.FNumber
      };
    },
    hasExif() {
      return Object.values(this.exifData).some(val => val !== undefined && val !== null);
    },
    aiTags() {
      if (!this.currentPhoto || !this.currentPhoto.objects) return [];
      return Object.entries(this.currentPhoto.objects)
        .map(([name, score]) => ({
          name,
          percent: Math.round(score * 100)
        }))
        .sort((a, b) => b.percent - a.percent);
    },
    uniquePeople() {
      if (!this.detectedFaces) return [];
      const seen = new Set();
      return this.detectedFaces.filter(face => {
        if (!face.person_id) return true;
        if (seen.has(face.person_id)) return false;
        seen.add(face.person_id);
        return true;
      });
    },
    modelChips() {
      if (!this.currentPhoto) return [];
      const status = this.currentPhoto.ai_status || {};
      return this.modelInfo
        .filter(m => {
          return this.downloadedModels.includes(m.id);
        })
        .map(m => ({
          id: m.id,
          label: this.$t('models.' + m.id + '.title'),
          done: status[m.id] === 1,
        }));
    },
  },
  methods: {
    isVideoPhoto(photo) {
      if (!photo || !photo.location) return false;
      const ext = photo.location.split('.').pop().toLowerCase();
      return ["mp4", "mkv", "mov", "avi", "webm"].includes(ext);
    },
    async fetchFaces() {
      if (!this.currentPhoto) return;
      try {
        const facesStr = await invoke("get_faces_for_photo", { photoId: this.currentPhoto.id });
        this.detectedFaces = JSON.parse(facesStr);
      } catch (e) {
        console.error("Failed to fetch faces", e);
      }
    },
    goToPerson(face) {
      if (!face.person_id) return;
      this.$emit('navigate-to-person', {
        id: face.person_id,
        name: face.person_name || this.$t('photo_viewer.unnamed')
      });
      this.close();
    },
    async analyzePhoto() {
      if (!this.currentPhoto || this.isAnalyzing || this.isAnalyzingModel) return;
      this.isAnalyzing = true;
      const photoId = this.currentPhoto.id;
      const startTime = Date.now();
      try {
        const { listen } = await import('@tauri-apps/api/event');
        const unlisten = await listen('photo-analysis-result', (event) => {
          if (event.payload.id === photoId) {
            this.isAnalyzing = false;
            this.fetchFaces();
            unlisten();

            const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);

            const r = event.payload;
            const parts = [];
            if (r.object_count > 0) parts.push(this.$t('photo_viewer.objects_count', { count: r.object_count }));
            if (r.face_count > 0) parts.push(this.$t('photo_viewer.faces_count', { count: r.face_count }));
            if (r.has_caption) parts.push(this.$t('photo_viewer.has_caption'));
            if (parts.length === 0) parts.push(this.$t('photo_viewer.nothing_detected'));

            this.snackbar.text = this.$t('photo_viewer.analysis_complete', { models: parts.join(', '), time: elapsed });
            this.snackbar.show = true;

            this.refreshPhoto(photoId);
            this.showInfo = true;
          }
        });
        await invoke("analyze_photo", { id: photoId });
      } catch (e) {
        console.error("Analysis failed", e);
        this.isAnalyzing = false;
      }
    },
    async runSingleModel(modelId) {
      if (!this.currentPhoto || this.isAnalyzing || this.isAnalyzingModel) return;
      console.log('runSingleModel start', modelId, 'photo:', this.currentPhoto.id);
      this.isAnalyzingModel = modelId;
      this.runStartTime = Date.now();
      this.runTimerTick = 0;
      this.runTimer = window.setInterval(() => {
        this.runTimerTick += 1;
      }, 1000);
      const photoId = this.currentPhoto.id;
      try {
        const { listen } = await import('@tauri-apps/api/event');
        const unlisten = await listen('photo-analysis-result', (event) => {
          console.log('runSingleModel got event', event.payload.id, 'expected', photoId, 'model_timings:', event.payload.model_timings);
          if (event.payload.id === photoId) {
            this.isAnalyzingModel = null;
            if (this.runTimer) { clearInterval(this.runTimer); this.runTimer = null; }
            this.fetchFaces();
            unlisten();
            this.refreshPhoto(photoId);
            this.showInfo = true;
            const modelTimings = event.payload.model_timings || {};
            const modelTime = modelTimings[modelId];
            const elapsed = (modelTime || ((Date.now() - this.runStartTime) / 1000)).toFixed(1);
            this.snackbar.text = this.$t('photo_viewer.model_complete', { model: this.$t('models.' + modelId + '.title'), time: elapsed });
            this.snackbar.error = false;
            this.snackbar.show = true;
          }
        });
        await invoke("analyze_photo_model", { id: photoId, modelId });
        console.log('runSingleModel invoke returned');
      } catch (e) {
        console.error("Model analysis failed", e);
        this.isAnalyzingModel = null;
        if (this.runTimer) { clearInterval(this.runTimer); this.runTimer = null; }
        this.snackbar.text = this.$t('photo_viewer.model_failed', { model: this.$t('models.' + modelId + '.title') });
        this.snackbar.error = true;
        this.snackbar.show = true;
      }
    },
    async refreshPhoto(photoId) {
      try {
        const photoJson = await invoke("get_photo_by_id", { id: photoId });
        if (!photoJson || photoJson === 'null') return;
        const updated = JSON.parse(photoJson);
        const idx = this.photos.findIndex(p => p.id === photoId);
        if (idx !== -1) {
          this.$emit('update:photo', updated);
        }
      } catch (e) {
        console.error("Failed to refresh photo", e);
      }
    },
    formatEta(ms) {
      if (!ms || ms < 0) return this.$t('photo_viewer.calculating');
      const totalSeconds = Math.floor(ms / 1000);
      const hours = Math.floor(totalSeconds / 3600);
      const minutes = Math.floor((totalSeconds % 3600) / 60);
      if (hours > 0) return `${hours}h ${minutes}m`;
      if (minutes > 0) return `${minutes}m`;
      return `${totalSeconds % 60}s`;
    },
    async listenForEta() {
      const { listen } = await import('@tauri-apps/api/event');
      this.unlistenEta = await listen('indexing-eta', (event) => {
        this.globalEta = event.payload;
      });
    },
    close() { this.visible = false; },
    formatElapsed(start, tick) {
      if (!start) return '0s';
      void tick;
      const sec = Math.floor((Date.now() - start) / 1000);
      if (sec < 60) return `${sec}s`;
      const m = Math.floor(sec / 60);
      return `${m}m ${sec % 60}s`;
    },
    next() {
        if (this.photos.length === 0) return;
        const newIndex = (this.index + 1) % this.photos.length;
        this.$emit('update:index', newIndex);
    },
    prev() {
        if (this.photos.length === 0) return;
        const newIndex = (this.index - 1 + this.photos.length) % this.photos.length;
        this.$emit('update:index', newIndex);
    },
    handleKeydown(e) {
        if (!this.visible) return;
        if (e.key === 'ArrowRight') this.next();
        if (e.key === 'ArrowLeft') this.prev();
        if (e.key === 'Escape') this.close();
        if (e.key === 'i') this.showInfo = !this.showInfo;
    },
    scrollToActiveThumb() {
      this.$nextTick(() => {
        const rail = this.$refs.thumbnailRail;
        if (!rail) return;
        const activeItem = rail.querySelector('.rail-item.active');
        if (activeItem) {
          activeItem.scrollIntoView({ behavior: 'smooth', inline: 'center', block: 'nearest' });
        }
      });
    }
  },
  watch: {
    index() {
      this.isAnalyzing = false;
      this.isAnalyzingModel = null;
      if (this.runTimer) { clearInterval(this.runTimer); this.runTimer = null; }
      this.fetchFaces();
      this.scrollToActiveThumb();
      if (this.isVideo) this.showInfo = false;
    },
    visible(val) {
      if (val) {
        this.fetchFaces();
        this.scrollToActiveThumb();
      } else {
        this.detectedFaces = [];
      }
    }
  },
  async mounted() {
      window.addEventListener('keydown', this.handleKeydown);
      try { this.os = await invoke("get_os"); } catch (e) {}
      if (!window.__pv_mediaPort) { try { window.__pv_mediaPort = await invoke("get_media_server_port"); } catch (e) {} } this.mediaPort = window.__pv_mediaPort;
      this.listenForEta();
      try { this.downloadedModels = await invoke("check_models"); } catch (e) {}
  },
  beforeUnmount() {
      window.removeEventListener('keydown', this.handleKeydown);
      if (this.unlistenEta) this.unlistenEta();
      if (this.unlistenResult) this.unlistenResult();
  }
}
</script>

<style scoped>
.viewer-content-container {
  flex: 1;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  overflow: hidden;
}

.media-wrapper {
  height: 100%;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1;
}

.viewer-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  transition: opacity 0.2s ease-in-out;
  user-select: none;
  -webkit-user-drag: none;
}

.touch-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 100px;
  z-index: 5;
}

/* Nav Buttons */
.viewer-nav-btn {
  position: absolute;
  z-index: 2000;
}
.top-left { top: 20px; left: 20px; }
.top-right { top: 20px; right: 20px; }

.side-nav-btn {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  z-index: 10;
  background: rgba(255,255,255,0.1);
  backdrop-filter: blur(4px);
  border-radius: 50%;
}
.side-nav-btn.left { left: 20px; }
.side-nav-btn.right { right: 20px; }

/* Thumbnail Rail */
.thumbnail-rail-container {
  width: 100%;
  height: 100px;
  background: var(--color-bg-rail);
  backdrop-filter: blur(12px);
  border-top: 1px solid var(--color-border-subtle);
  display: flex;
  align-items: center;
  padding: 0 10px;
  z-index: 20;
}

.thumbnail-rail {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  padding: 10px 0;
  width: 100%;
  scrollbar-width: none;
}
.thumbnail-rail::-webkit-scrollbar { display: none; }

.rail-item {
  min-width: 60px;
  height: 60px;
  border-radius: 8px;
  overflow: hidden;
  cursor: pointer;
  position: relative;
  border: 2px solid transparent;
  transition: all 0.2s ease;
  opacity: 0.6;
}

.rail-item.active {
  border-color: #000000;
  opacity: 1;
  transform: scale(1.1);
}

.rail-item img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.rail-video-icon {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0,0,0,0.2);
}

.info-drawer {
  border-left: 1px solid rgba(0,0,0,0.05);
  z-index: 3000;
}

.tracking-widest {
  letter-spacing: 0.1em;
}

.analyzing-dots::after {
  content: '';
  animation: dots 1.5s steps(4, end) infinite;
}

@keyframes dots {
  0%   { content: ''; }
  25%  { content: '.'; }
  50%  { content: '..'; }
  75%  { content: '...'; }
  100% { content: ''; }
}
</style>