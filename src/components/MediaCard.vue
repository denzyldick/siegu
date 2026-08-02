<template>
  <div
    class="media-card-container"
    ref="containerRef"
    :class="{ 'is-selected': selected, 'selection-active': selectionMode }"
    @click="handleClick"
  >
    <div class="media-card-wrapper shadow-sm">
      <template v-if="isVisible">
        <video
          v-if="isVideo && !path.encoded && computedVideoUrl"
          :src="computedVideoUrl + '#t=0.5'"
          class="media-card-img"
          muted
          preload="metadata"
          @error="onVideoError"
        ></video>
        <img
          v-else
          :src="imageSrc"
          loading="lazy"
          :alt="$t('media_card.alt_photo')"
          class="media-card-img"
          @error="onImageError"
        />

        <div class="scrim-overlay"></div>

        <button
          v-if="notSynced"
          class="action-btn not-synced-badge"
          :title="$t('media_card.not_synced')"
          @click.stop="$emit('not-synced')"
        >
          <v-icon size="14" color="white">mdi-cloud-upload-outline</v-icon>
        </button>

        <div v-if="isVideo" class="video-indicator">
          <v-icon color="white" size="20">mdi-play</v-icon>
        </div>

        <div v-if="nsfwScore >= NSFW_THRESHOLD" class="nsfw-badge" :title="$t('media_card.nsfw')">
          <v-icon size="14" color="white">mdi-alert-octagon</v-icon>
          <span>{{ Math.round(nsfwScore * 100) }}%</span>
        </div>

        <div v-if="selectionMode" class="selection-indicator">
          <div class="check-circle" :class="{ checked: selected }">
            <v-icon v-if="selected" color="white" size="16">mdi-check</v-icon>
          </div>
        </div>

        <button
          v-if="!selectionMode"
          class="action-btn favorite-action"
          :class="{ 'is-fav': path.favorite }"
          @click.stop="toggleFavorite"
        >
          <v-icon size="18" :color="path.favorite ? '#ef4444' : 'white'">
            {{ path.favorite ? 'mdi-heart' : 'mdi-heart-outline' }}
          </v-icon>
        </button>
      </template>
      <div v-else class="viewport-placeholder h-100 w-100 d-flex align-center justify-center"></div>
    </div>
    <div v-if="!selectionMode" class="media-card-info">
      <div class="media-card-info-top">
        <div class="media-card-tags" v-if="tags.length > 0">
          <span v-for="tag in tags" :key="tag" class="info-tag">{{ tag }}</span>
        </div>
        <div class="media-card-meta" v-if="hasResults">
          <v-icon size="12" color="#a1a1aa">mdi-auto-fix</v-icon>
        </div>
      </div>
      <div
        class="media-card-caption click-caption"
        v-if="path.caption"
        @click.stop="$emit('click')"
      >
        {{ path.caption }}
      </div>
      <div class="media-card-details" v-if="hasResults">
        <span
          v-if="path.aesthetics_score != null"
          class="detail-item"
          :title="$t('media_card.aesthetics_score')"
        >
          <v-icon size="10" color="#a1a1aa">mdi-star</v-icon>
          {{ formatScore(path.aesthetics_score) }}
        </span>
        <span v-if="faceCount > 0" class="detail-item" :title="$t('media_card.faces_detected')">
          <v-icon size="10" color="#a1a1aa">mdi-face</v-icon>
          {{ faceCount }}
        </span>
        <span v-if="path.indexed === 2" class="detail-item" :title="$t('media_card.fully_indexed')">
          <v-icon size="10" color="#22c55e">mdi-check-circle</v-icon>
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useMediaUrl } from '@/composables/useMediaUrl';
import { isVideo as checkIsVideo, formatScore } from '@/composables/useMediaUtils';
import type { MediaItem } from '@/types/media';

const props = defineProps<{
  path: MediaItem;
  selected: boolean;
  selectionMode: boolean;
}>();

const emit = defineEmits<{
  'toggle-favorite': [id: string | number];
  'not-synced': [];
  click: [];
  select: [id: string | number];
}>();

const { videoUrl: buildVideoUrl } = useMediaUrl();

const containerRef = ref<HTMLElement | null>(null);
const isVisible = ref(false);
let observer: IntersectionObserver | null = null;

const isVideo = computed(() => {
  if (!props.path?.location) return false;
  return checkIsVideo(props.path.location);
});

const computedVideoUrl = computed(() => {
  if (!props.path?.location || !isVideo.value) return '';
  return buildVideoUrl(props.path.location);
});

const imageSrc = computed(() => {
  if (!props.path?.location) return undefined;
  if (props.path.encoded) return props.path.encoded;
  if (!isVideo.value) {
    const ext = props.path.location.split('.').pop()?.toLowerCase();
    if (['heic', 'heif'].includes(ext ?? '')) return undefined;
    return convertFileSrc(props.path.location);
  }
  return undefined;
});

const tags = computed((): string[] => {
  if (!props.path?.objects) return [];
  return Object.entries(props.path.objects)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 3)
    .map((entry) => entry[0]);
});

const faceCount = computed((): number => {
  if (!props.path?.properties) return 0;
  const v = props.path.properties['face_count'];
  return v ? parseInt(String(v)) : 0;
});

const NSFW_THRESHOLD = 0.8;

const nsfwScore = computed((): number => {
  if (!props.path?.properties) return 0;
  const v = props.path.properties['nsfw'];
  const score = parseFloat(String(v));
  return Number.isFinite(score) ? score : 0;
});

const notSynced = computed((): boolean => !!props.path?.sync_needed && !props.path?.received);

const hasResults = computed((): boolean => {
  if (!props.path) return false;
  return (
    (props.path.objects && Object.keys(props.path.objects).length > 0) ||
    props.path.aesthetics_score != null ||
    !!props.path.caption ||
    props.path.indexed >= 2
  );
});

function toggleFavorite(): void {
  emit('toggle-favorite', props.path.id);
}

function handleClick(): void {
  if (props.selectionMode) {
    emit('select', props.path.id);
  } else {
    emit('click');
  }
}

function onImageError(): void {
  const ext = props.path?.location?.split('.').pop()?.toLowerCase();
  if (['heic', 'heif'].includes(ext ?? '') && !props.path?.encoded) return;
  console.error('[MediaCard] Failed to load:', props.path?.location);
}

function onVideoError(): void {
  console.error('[MediaCard] Failed to load video:', props.path?.location);
}

onMounted(() => {
  if (typeof IntersectionObserver === 'undefined') {
    isVisible.value = true;
    return;
  }
  observer = new IntersectionObserver(
    (entries) => {
      isVisible.value = entries[0].isIntersecting;
    },
    { rootMargin: '200px', threshold: 0.01 },
  );
  if (containerRef.value) observer.observe(containerRef.value);
});

onUnmounted(() => {
  observer?.disconnect();
});
</script>

<style scoped>
.media-card-container {
  width: 100%;
  position: relative;
  cursor: pointer;
  transition: transform 0.4s cubic-bezier(0.16, 1, 0.3, 1);
  will-change: transform;
}

.media-card-wrapper {
  width: 100%;
  aspect-ratio: 1;
  overflow: hidden;
  border-radius: 16px;
  position: relative;
  background-color: #f4f4f5;
  border: 1px solid rgba(0, 0, 0, 0.05);
}

.viewport-placeholder {
  background-color: #f4f4f5;
}

.media-card-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.6s cubic-bezier(0.16, 1, 0.3, 1);
}

.scrim-overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    to bottom,
    rgba(0, 0, 0, 0.2) 0%,
    transparent 30%,
    transparent 70%,
    rgba(0, 0, 0, 0.3) 100%
  );
  opacity: 0;
  transition: opacity 0.3s ease;
  z-index: 1;
}

.media-card-container:hover .scrim-overlay {
  opacity: 1;
}

.media-card-container:hover .media-card-img {
  transform: scale(1.08);
}

.media-card-container:active {
  transform: scale(0.96);
}

.selection-active .media-card-wrapper {
  transform: scale(0.92);
}

.is-selected .media-card-wrapper {
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
  background: rgba(0, 0, 0, 0.2);
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

.video-indicator {
  position: absolute;
  bottom: 12px;
  right: 12px;
  width: 32px;
  height: 32px;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(8px);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 5;
}

.nsfw-badge {
  position: absolute;
  bottom: 12px;
  left: 12px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
  font-size: 10px;
  font-weight: 700;
  color: white;
  background: rgba(220, 38, 38, 0.9);
  backdrop-filter: blur(8px);
  border-radius: 999px;
  padding: 3px 8px;
  z-index: 5;
}

.action-btn {
  position: absolute;
  top: 12px;
  right: 12px;
  width: 32px;
  height: 32px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.2);
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

.media-card-container:hover .action-btn,
.action-btn.is-fav {
  opacity: 1;
  transform: translateY(0);
}

.action-btn.is-fav {
  background: white;
}

.not-synced-badge {
  position: absolute;
  top: 12px;
  left: 12px;
  right: auto;
  width: 28px;
  height: 28px;
  background: rgba(245, 158, 11, 0.9);
  opacity: 1;
  transform: none;
  z-index: 6;
}

.not-synced-badge:hover {
  background: #d97706;
}

.shadow-sm {
  box-shadow: 0 1px 2px 0 rgba(0, 0, 0, 0.05);
}

.media-card-info {
  margin-top: 6px;
  padding: 0 2px;
}

.media-card-info-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
}

.media-card-tags {
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

.media-card-meta {
  flex-shrink: 0;
  opacity: 0.5;
}

.media-card-caption {
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

.media-card-details {
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
