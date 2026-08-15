<template>
  <div
    class="media-card-container"
    ref="containerRef"
    :class="{ 'is-selected': selected, 'selection-active': selectionMode }"
    @click="handleClick"
  >
    <div class="media-card-wrapper elevation-1">
      <template v-if="isVisible">
        <img
          v-if="imageSrc"
          :src="imageSrc"
          loading="lazy"
          :alt="$t('media_card.alt_photo')"
          class="media-card-img"
          @error="onImageError"
        />
        <img
          v-else-if="posterSrc"
          :src="posterSrc"
          loading="lazy"
          :alt="$t('media_card.alt_photo')"
          class="media-card-img"
          @error="onPosterError"
        />
        <video
          v-else-if="isVideo && !path.encoded && computedVideoUrl"
          :src="computedVideoUrl + '#t=0.5'"
          class="media-card-img"
          muted
          preload="metadata"
          @error="onVideoError($event)"
        ></video>
        <div v-else class="media-card-img img-placeholder"></div>

        <div class="scrim-overlay"></div>

        <v-btn
          v-if="notSynced"
          variant="flat"
          class="action-btn not-synced-badge"
          :title="$t('media_card.not_synced')"
          @click.stop="$emit('not-synced')"
        >
          <v-icon size="14" color="white">mdi-cloud-upload-outline</v-icon>
        </v-btn>

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

        <v-btn
          v-if="!selectionMode"
          variant="flat"
          class="action-btn favorite-action"
          :class="{ 'is-fav': path.favorite }"
          @click.stop="toggleFavorite"
        >
          <v-icon size="18" :color="path.favorite ? 'rgb(var(--v-theme-error))' : 'white'">
            {{ path.favorite ? 'mdi-heart' : 'mdi-heart-outline' }}
          </v-icon>
        </v-btn>
      </template>
      <div v-else class="viewport-placeholder h-100 w-100 d-flex align-center justify-center"></div>
    </div>
    <div v-if="!selectionMode" class="media-card-info">
      <div class="media-card-info-top">
        <div class="media-card-tags" v-if="tags.length > 0">
          <span v-for="tag in tags" :key="tag" class="info-tag">{{ tag }}</span>
        </div>
        <div class="media-card-meta" v-if="hasResults">
          <v-icon size="12" color="rgba(var(--v-theme-on-surface), 0.7)">mdi-auto-fix</v-icon>
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
          <v-icon size="10" color="rgba(var(--v-theme-on-surface), 0.7)">mdi-star</v-icon>
          {{ formatScore(path.aesthetics_score) }}
        </span>
        <span v-if="faceCount > 0" class="detail-item" :title="$t('media_card.faces_detected')">
          <v-icon size="10" color="rgba(var(--v-theme-on-surface), 0.7)">mdi-face</v-icon>
          {{ faceCount }}
        </span>
        <span v-if="path.indexed === 2" class="detail-item" :title="$t('media_card.fully_indexed')">
          <v-icon size="10" color="rgb(var(--v-theme-success))">mdi-check-circle</v-icon>
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
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

const { videoUrl: buildVideoUrl, thumbUrl: buildThumbUrl } = useMediaUrl();

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

const posterFailed = ref(false);

const imageSrc = computed(() => {
  if (!props.path?.location || isVideo.value) return undefined;
  const thumb = buildThumbUrl(props.path.location);
  if (thumb) return thumb;
  return props.path.encoded || undefined;
});

// Prefer a generated poster (320px thumb from ffmpeg) over mounting a video
// element per card, which forces the browser to download media bytes.
const posterSrc = computed(() => {
  if (posterFailed.value || !isVideo.value || !props.path?.location) return undefined;
  return buildThumbUrl(props.path.location);
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

function onVideoError(event: Event): void {
  const target = event.target as HTMLMediaElement | null;
  console.error('[MediaCard] Failed to load video:', {
    src: target?.currentSrc || target?.src || null,
    code: target?.error?.code ?? null,
    location: props.path?.location,
  });
}

function onPosterError(): void {
  posterFailed.value = true;
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
  border-radius: 24px;
  position: relative;
  background-color: rgb(var(--v-theme-surface-light));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
}

.viewport-placeholder {
  background-color: rgb(var(--v-theme-surface-light));
}

.img-placeholder {
  background-color: rgb(var(--v-theme-surface-light));
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
  border: 4px solid rgb(var(--v-theme-primary));
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
  background: rgb(var(--v-theme-primary));
  border-color: rgb(var(--v-theme-primary));
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
  background: color-mix(in srgb, rgb(var(--v-theme-error)) 90%, transparent);
  backdrop-filter: blur(8px);
  border-radius: 9999px;
  padding: 3px 8px;
  z-index: 5;
}

.action-btn {
  position: absolute;
  top: 12px;
  right: 12px;
  width: 32px;
  height: 32px;
  border-radius: 8px;
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
  background: color-mix(in srgb, rgb(var(--v-theme-warning)) 90%, transparent);
  opacity: 1;
  transform: none;
  z-index: 6;
}

.not-synced-badge:hover {
  background: rgb(var(--v-theme-warning));
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
  color: rgba(var(--v-theme-on-surface), 0.6);
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
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
  color: rgba(var(--v-theme-on-surface), 0.7);
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.click-caption {
  cursor: pointer;
}

.click-caption:hover {
  color: rgb(var(--v-theme-on-surface));
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
  color: rgba(var(--v-theme-on-surface), 0.7);
  display: inline-flex;
  align-items: center;
  gap: 2px;
}
</style>
