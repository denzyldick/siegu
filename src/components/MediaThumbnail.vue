<template>
  <div class="rail-item" ref="container" :class="{ active: active }" @click="$emit('click')">
    <template v-if="isVisible">
      <img v-if="thumbSrc" :src="thumbSrc" :alt="$t('media_thumbnail.alt_thumb')" />
      <video
        v-else-if="isVideo && !photo.encoded"
        :src="videoUrl"
        :type="videoType"
        :alt="$t('media_thumbnail.alt_thumb')"
        muted
        playsinline
        preload="metadata"
      />
      <div v-if="isVideo" class="rail-video-icon">
        <v-icon size="12" color="white">mdi-play</v-icon>
      </div>
    </template>
    <div v-else class="rail-placeholder"></div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useMediaUrl } from '@/composables/useMediaUrl';
import type { MediaItem } from '@/types/media';

const props = defineProps<{
  photo: MediaItem;
  active: boolean;
}>();

defineEmits<{
  click: [];
}>();

const { isVideo: checkIsVideo, videoUrl: buildVideoUrl, thumbUrl: buildThumbUrl } = useMediaUrl();

const isVisible = ref(false);
const container = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;

const isVideo = computed(() => {
  if (!props.photo?.location) return false;
  return checkIsVideo(props.photo.location);
});

const videoUrl = computed(() => {
  if (!props.photo?.location || !isVideo.value) return '';
  return buildVideoUrl(props.photo.location) ?? '';
});

const videoType = computed(() => {
  const ext = props.photo?.location?.split('.').pop()?.toLowerCase();
  if (ext === 'mp4') return 'video/mp4';
  if (ext === 'webm') return 'video/webm';
  if (ext === 'mov') return 'video/mp4';
  if (ext === 'mkv') return 'video/x-matroska';
  if (ext === 'm4v') return 'video/mp4';
  return undefined;
});

const thumbSrc = computed(() => {
  if (!props.photo?.location) return '';
  if (props.photo.encoded) return props.photo.encoded;
  return buildThumbUrl(props.photo.location) ?? '';
});

onMounted(() => {
  if (typeof IntersectionObserver === 'undefined') {
    isVisible.value = true;
    return;
  }
  observer = new IntersectionObserver(
    (entries) => {
      isVisible.value = entries[0].isIntersecting;
    },
    { rootMargin: '100px', threshold: 0.01 },
  );
  if (container.value) observer.observe(container.value);
});

onUnmounted(() => {
  observer?.disconnect();
});
</script>

<style scoped>
.rail-item {
  width: 60px;
  height: 60px;
  border-radius: 8px;
  overflow: hidden;
  cursor: pointer;
  position: relative;
  border: 2px solid transparent;
  transition: all 0.2s ease;
  opacity: 0.6;
  background: rgb(var(--v-theme-surface-light));
}

.rail-item.active {
  border-color: rgb(var(--v-theme-primary));
  opacity: 1;
  transform: scale(1.1);
}

.rail-item img,
.rail-item video {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.rail-placeholder {
  width: 100%;
  height: 100%;
  background: rgb(var(--v-theme-surface-light));
}

.rail-video-icon {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.2);
}
</style>
