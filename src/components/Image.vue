<template>
    <div
      class="image-item-container"
      ref="container"
      :class="{ 'is-selected': selected, 'selection-active': selectionMode }"
      @click="handleClick"
    >
      <div class="image-wrapper shadow-sm">
        <template v-if="isVisible">
          <video v-if="isVideo" :src="videoUrl + '#t=0.5'" class="photo-img" muted preload="metadata"></video>
          <img v-else :src="imageSrc" loading="lazy" :alt="$t('image.alt_photo')" class="photo-img" @error="onImageError" />
          
          <div class="scrim-overlay"></div>

          <!-- Video Indicator -->
          <div v-if="isVideo" class="video-indicator">
            <v-icon color="white" size="20">mdi-play</v-icon>
          </div>

          <!-- Selection Mode UI -->
          <div v-if="selectionMode" class="selection-indicator">
            <div class="check-circle" :class="{ 'checked': selected }">
              <v-icon v-if="selected" color="white" size="16">mdi-check</v-icon>
            </div>
          </div>

          <!-- Favorite Button -->
          <button
            v-if="!selectionMode"
            class="action-btn favorite-action"
            :class="{ 'is-fav': isFavorite }"
            @click.stop="toggleFavorite"
          >
            <v-icon size="18" :color="isFavorite ? '#ef4444' : 'white'">
              {{ isFavorite ? 'mdi-heart' : 'mdi-heart-outline' }}
            </v-icon>
          </button>
        </template>
        <div v-else class="viewport-placeholder h-100 w-100 d-flex align-center justify-center">
        </div>
      </div>
      <!-- Image Info Footer -->
      <div v-if="!selectionMode" class="image-info">
        <div class="image-info-top">
          <div class="image-tags" v-if="tags.length > 0">
            <span v-for="tag in tags" :key="tag" class="info-tag">{{ tag }}</span>
          </div>
          <div class="image-meta" v-if="hasResults">
            <v-icon size="12" color="#a1a1aa">mdi-auto-fix</v-icon>
          </div>
        </div>
        <div class="image-caption click-caption" v-if="path.caption" @click.stop="$emit('click')">
          {{ path.caption }}
        </div>
        <div class="image-details" v-if="hasResults">
          <span v-if="path.aesthetics_score != null" class="detail-item" :title="$t('image.aesthetics_score')">
            <v-icon size="10" color="#a1a1aa">mdi-star</v-icon>
            {{ formatScore(path.aesthetics_score) }}
          </span>
          <span v-if="faceCount > 0" class="detail-item" :title="$t('image.faces_detected')">
            <v-icon size="10" color="#a1a1aa">mdi-face</v-icon>
            {{ faceCount }}
          </span>
          <span v-if="path.indexed === 2" class="detail-item" :title="$t('image.fully_indexed')">
            <v-icon size="10" color="#22c55e">mdi-check-circle</v-icon>
          </span>
        </div>
      </div>
    </div>
</template>

<script>
import { convertFileSrc, invoke } from '@tauri-apps/api/core';

export default {
  name: "Image",
  props: {
    path: Object,
    selected: Boolean,
    selectionMode: Boolean
  },
  emits: ['toggle-favorite', 'click', 'select'],
  data: () => ({
    mediaPort: null,
    isVisible: false,
    observer: null
  }),
  async mounted() {
    this.setupObserver();
    if (!window.__img_mediaPort) {
      try {
        window.__img_mediaPort = await invoke("get_media_server_port");
      } catch (e) {}
    }
    this.mediaPort = window.__img_mediaPort;
  },
  computed: {
    videoUrl() {
      if (!this.path || !this.isVideo || !this.mediaPort) return '';
      let path = this.path.location.replace(/\\/g, '/');
      if (path.match(/^[a-zA-Z]:\//)) {
          path = path.substring(3);
      } else if (path.startsWith('/')) {
          path = path.substring(1);
      }
      const encoded = path.split('/').map(encodeURIComponent).join('/');
      return `http://127.0.0.1:${this.mediaPort}/media/${encoded}`;
    },
    imageSrc() {
      if (!this.path || !this.path.location) return null;
      if (this.path.encoded && !this.isVideo) {
        return this.path.encoded;
      }
      if (!this.isVideo) {
        const ext = this.path.location.split('.').pop().toLowerCase();
        if (['heic', 'heif'].includes(ext)) {
          return null;
        }
        return convertFileSrc(this.path.location);
      }
      return null;
    },
    isFavorite() {
        return this.path.favorite === true;
    },
    isVideo() {
      if (!this.path || !this.path.location) return false;
      const ext = this.path.location.split('.').pop().toLowerCase();
      return ["mp4", "mkv", "mov", "avi", "webm"].includes(ext);
    },
    tags() {
      if (!this.path || !this.path.objects) return [];
      return Object.entries(this.path.objects)
        .sort((a, b) => b[1] - a[1])
        .slice(0, 3)
        .map(entry => entry[0]);
    },
    faceCount() {
      if (!this.path || !this.path.properties) return 0;
      const v = this.path.properties['face_count'];
      return v ? parseInt(v) : 0;
    },
    hasResults() {
      if (!this.path) return false;
      return (this.path.objects && Object.keys(this.path.objects).length > 0)
        || this.path.aesthetics_score != null
        || this.path.caption
        || this.path.indexed >= 2;
    }
  },
  methods: {
      setupObserver() {
        this.observer = new IntersectionObserver((entries) => {
          this.isVisible = entries[0].isIntersecting;
        }, {
          rootMargin: '200px',
          threshold: 0.01
        });
        
        if (this.$refs.container) {
          this.observer.observe(this.$refs.container);
        }
      },
      toggleFavorite() {
          this.$emit('toggle-favorite', this.path.id);
      },
      formatScore(score) {
        if (score == null) return '';
        const v = typeof score === 'string' ? parseFloat(score) : score;
        return v.toFixed(2);
      },
      onImageError(e) {
        const ext = this.path?.location?.split('.').pop().toLowerCase();
        if (['heic', 'heif'].includes(ext) && !this.path?.encoded) {
          return;
        }
        console.error('[Image] Failed to load:', this.path?.location, 'encoded:', this.path?.encoded ? 'yes' : 'no', 'src type:', this.path?.encoded ? 'base64' : 'convertFileSrc');
      },
      handleClick() {
        if (this.selectionMode) {
          this.$emit('select', this.path.id);
        } else {
          this.$emit('click');
        }
      }
  }
};
</script>

<style scoped>
.image-item-container {
  width: 100%;
  position: relative;
  cursor: pointer;
  transition: transform 0.4s cubic-bezier(0.16, 1, 0.3, 1);
  will-change: transform;
}

.image-wrapper {
  width: 100%;
  aspect-ratio: 1;
  overflow: hidden;
  border-radius: 16px;
  position: relative;
  background-color: #f4f4f5;
  border: 1px solid rgba(0,0,0,0.05);
}

.viewport-placeholder {
  background-color: #f4f4f5;
}

.photo-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.6s cubic-bezier(0.16, 1, 0.3, 1);
}

.video-placeholder {
  width: 100%;
  height: 100%;
  background-color: #f4f4f5;
}

.scrim-overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(to bottom, rgba(0,0,0,0.2) 0%, transparent 30%, transparent 70%, rgba(0,0,0,0.3) 100%);
  opacity: 0;
  transition: opacity 0.3s ease;
  z-index: 1;
}

.image-item-container:hover .scrim-overlay {
  opacity: 1;
}

.image-item-container:hover .photo-img {
  transform: scale(1.08);
}

.image-item-container:active {
  transform: scale(0.96);
}

/* Selection State */
.selection-active .image-wrapper {
  transform: scale(0.92);
}

.is-selected .image-wrapper {
  border: 4px solid #000000;
  transform: scale(0.92);
}

.selection-indicator {
  position: absolute;
  top: 12px;
  left: 12px;
  z-index: 10;
}

.check-circle {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 2px solid white;
  background: rgba(0,0,0,0.2);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.check-circle.checked {
  background: #000000;
  border-color: #000000;
}

/* Video Indicator */
.video-indicator {
  position: absolute;
  bottom: 12px;
  right: 12px;
  width: 32px;
  height: 32px;
  background: rgba(0,0,0,0.5);
  backdrop-filter: blur(8px);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 5;
}

/* Favorite Button */
.action-btn {
  position: absolute;
  top: 12px;
  right: 12px;
  width: 32px;
  height: 32px;
  border-radius: 10px;
  background: rgba(255,255,255,0.2);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  cursor: pointer;
  opacity: 0;
  transform: translateY(-4px);
  transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  z-index: 5;
}

.image-item-container:hover .action-btn,
.action-btn.is-fav {
  opacity: 1;
  transform: translateY(0);
}

.action-btn.is-fav {
  background: white;
}

/* AI Tags */
.ai-tags-preview {
  position: absolute;
  bottom: 12px;
  left: 12px;
  display: flex;
  gap: 4px;
  z-index: 5;
  opacity: 0;
  transform: translateY(4px);
  transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}

.image-item-container:hover .ai-tags-preview {
  opacity: 1;
  transform: translateY(0);
}

.tag-pill {
  font-size: 10px;
  font-weight: 700;
  color: white;
  background: rgba(0,0,0,0.5);
  backdrop-filter: blur(4px);
  padding: 2px 8px;
  border-radius: 6px;
  text-transform: capitalize;
}

.shadow-sm {
  box-shadow: 0 1px 2px 0 rgba(0, 0, 0, 0.05);
}

/* Image Info Footer */
.image-info {
  margin-top: 6px;
  padding: 0 2px;
}

.image-info-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
}

.image-tags {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  min-width: 0;
  overflow: hidden;
}

.info-tag {
  font-size: 10px;
  font-weight: 600;
  color: #71717a;
  background: #f4f4f5;
  padding: 1px 6px;
  border-radius: 4px;
  text-transform: capitalize;
  white-space: nowrap;
}

.image-meta {
  flex-shrink: 0;
  opacity: 0.5;
}

.image-caption {
  font-size: 11px;
  color: #52525b;
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.click-caption {
  cursor: pointer;
}

.click-caption:hover {
  color: #18181b;
  text-decoration: underline;
}

.image-details {
  display: flex;
  gap: 8px;
  margin-top: 2px;
  flex-wrap: wrap;
}

.detail-item {
  font-size: 10px;
  color: #a1a1aa;
  display: inline-flex;
  align-items: center;
  gap: 2px;
}
</style>
