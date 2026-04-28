<template>
  <v-dialog v-model="visible" fullscreen transition="dialog-bottom-transition">
    <v-card rounded="0" color="#fafafa" class="fill-height" style="overflow: hidden;">
      <v-layout class="fill-height">
        <!-- Main Viewer Area -->
        <v-main class="fill-height position-relative d-flex flex-column align-center justify-center p-0" style="background-color: #fafafa;">

          <!-- Top Controls -->
          <v-btn icon="mdi-close" variant="text" color="#18181b" class="viewer-nav-btn top-left" @click="close"></v-btn>
          <v-btn
            v-if="!isVideo"
            icon="mdi-information-outline"
            variant="text"
            :color="showInfo ? '#18181b' : '#71717a'"
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
          v-model="showInfo"
          location="right"
          width="350"
          color="#ffffff"
          class="border-s border-subtle info-drawer"
        >
        <v-toolbar color="transparent" density="compact">
          <v-toolbar-title class="text-zinc-primary text-subtitle-1 font-weight-bold">Metadata</v-toolbar-title>
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
               Analyze Photo
             </v-btn>
             <div v-if="globalEta" class="text-caption text-zinc-muted mt-2 text-center">
                Library indexing: {{ formatEta(globalEta) }} remaining
             </div>
          </div>

          <div class="mb-6 pt-4">
            <div class="text-caption text-zinc-muted mb-1 text-uppercase tracking-widest">File Details</div>
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
            <div class="text-caption text-zinc-muted mb-1 text-uppercase tracking-widest">AI Caption</div>
            <div class="text-body-2 text-zinc-primary font-italic">
              "{{ currentPhoto.caption }}"
            </div>
          </div>

          <v-divider class="opacity-5 mb-4" v-if="currentPhoto?.caption"></v-divider>

          <div class="mb-6" v-if="hasExif">
            <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">Camera Settings</div>

            <div class="d-flex align-center mb-4" v-if="exifData.make || exifData.model">
              <v-icon size="small" color="#71717a" class="mr-2">mdi-camera</v-icon>
              <span class="text-body-2 text-zinc-secondary">{{ exifData.make }} {{ exifData.model }}</span>
            </div>

            <v-row dense>
              <v-col cols="6" v-if="exifData.date" class="mb-3">
                <div class="text-caption text-zinc-muted">Date Taken</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.date }}</div>
              </v-col>
              <v-col cols="6" v-if="exifData.dimensions" class="mb-3">
                <div class="text-caption text-zinc-muted">Resolution</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.dimensions }}</div>
              </v-col>
              <v-col cols="6" v-if="exifData.iso" class="mb-3">
                <div class="text-caption text-zinc-muted">ISO</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.iso }}</div>
              </v-col>
              <v-col cols="6" v-if="exifData.shutter" class="mb-3">
                <div class="text-caption text-zinc-muted">Shutter</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.shutter }}</div>
              </v-col>
              <v-col cols="6" v-if="exifData.aperture" class="mb-3">
                <div class="text-caption text-zinc-muted">Aperture</div>
                <div class="text-body-2 text-zinc-secondary">{{ exifData.aperture }}</div>
              </v-col>
            </v-row>
          </div>

          <v-divider class="opacity-5 mb-4"></v-divider>

          <div class="mb-6">
            <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">People in this Photo</div>
            <div v-if="detectedFaces.length === 0" class="text-body-2 text-zinc-muted font-italic">
              No faces detected.
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
                  {{ face.person_name || 'Unnamed' }}
                </div>
              </div>
            </div>
          </div>

          <v-divider class="opacity-5 mb-4"></v-divider>

          <div class="mb-6">
            <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">AI Insights</div>

            <div v-if="aiTags.length === 0" class="text-body-2 text-zinc-muted font-italic">
              No AI insights generated yet.
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
  emits: ['update:modelValue', 'update:index'],
  data: () => ({
    showInfo: false,
    os: '',
    mediaPort: null,
    detectedFaces: [],
    isAnalyzing: false,
    globalEta: 0,
    unlistenEta: null,
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
        if (!face.person_id) return true; // Show all if not yet clustered
        if (seen.has(face.person_id)) return false;
        seen.add(face.person_id);
        return true;
      });
    }
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
        name: face.person_name || 'Unnamed Person'
      });
      this.close();
    },
    async analyzePhoto() {
      if (!this.currentPhoto || this.isAnalyzing) return;
      this.isAnalyzing = true;
      try {
        await invoke("analyze_photo", { id: this.currentPhoto.id });
        // The worker will emit photo-received via WebRTC sync logic or we can listen for general updates
        // For simple local feedback, we poll or wait a bit then refresh faces
        setTimeout(() => {
           this.fetchFaces();
           this.isAnalyzing = false;
        }, 2000);
      } catch (e) {
        console.error("Analysis failed", e);
        this.isAnalyzing = false;
      }
    },
    formatEta(ms) {
      if (!ms || ms < 0) return 'calculating...';
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
      try { this.mediaPort = await invoke("get_media_server_port"); } catch (e) { console.error("Failed to get media server port", e); }
      this.listenForEta();
  },
  beforeUnmount() {
      window.removeEventListener('keydown', this.handleKeydown);
      if (this.unlistenEta) this.unlistenEta();
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
  background: rgba(255, 255, 255, 0.8);
  backdrop-filter: blur(12px);
  border-top: 1px solid rgba(0,0,0,0.05);
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
</style>