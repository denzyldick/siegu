<template>
  <div
    class="rail-item"
    ref="container"
    :class="{ 'active': active }"
    @click="$emit('click')"
  >
    <template v-if="isVisible">
      <video v-if="isVideo" :src="videoUrl + '#t=0.5'" alt="thumb" muted preload="metadata" />
      <img v-else :src="imageSrc" alt="thumb" />
      <div v-if="isVideo" class="rail-video-icon">
        <v-icon size="12" color="white">mdi-play</v-icon>
      </div>
    </template>
    <div v-else class="rail-placeholder"></div>
  </div>
</template>

<script>
import { convertFileSrc, invoke } from '@tauri-apps/api/core';

export default {
  name: "RailItem",
  props: {
    photo: Object,
    active: Boolean
  },
  emits: ['click'],
  data: () => ({
    isVisible: false,
    observer: null,
    mediaPort: null
  }),
  computed: {
    isVideo() {
      if (!this.photo || !this.photo.location) return false;
      const ext = this.photo.location.split('.').pop().toLowerCase();
      return ["mp4", "mkv", "mov", "avi", "webm"].includes(ext);
    },
    videoUrl() {
      if (!this.photo || !this.isVideo || !this.mediaPort) return '';
      let path = this.photo.location.replace(/\\/g, '/');
      if (path.match(/^[a-zA-Z]:\//)) {
          path = path.substring(3);
      } else if (path.startsWith('/')) {
          path = path.substring(1);
      }
      const encoded = path.split('/').map(encodeURIComponent).join('/');
      return `http://127.0.0.1:${this.mediaPort}/media/${encoded}`;
    },
    imageSrc() {
      if (!this.photo || !this.photo.location) return '';
      if (this.photo.encoded && !this.isVideo) return this.photo.encoded;
      return convertFileSrc(this.photo.location);
    }
  },
  async mounted() {
    try {
      this.mediaPort = await invoke("get_media_server_port");
    } catch (e) {}

    this.observer = new IntersectionObserver((entries) => {
      this.isVisible = entries[0].isIntersecting;
    }, {
      rootMargin: '100px',
      threshold: 0.01
    });
    if (this.$refs.container) {
      this.observer.observe(this.$refs.container);
    }
  },
  beforeUnmount() {
    if (this.observer) {
      this.observer.disconnect();
    }
  }
}
</script>

<style scoped>
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
  background: #f4f4f5;
}

.rail-item.active {
  border-color: #000000;
  opacity: 1;
  transform: scale(1.1);
}

.rail-item img, .rail-item video {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.rail-placeholder {
  width: 100%;
  height: 100%;
  background: #f4f4f5;
}

.rail-video-icon {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0,0,0,0.2);
}
</style>
