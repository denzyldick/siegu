<template>
  <div
    class="media-card-container"
    ref="containerRef"
    :class="{ 'is-selected': selected, 'selection-active': selectionMode }"
    @click="handleClick"
  >
    <div
      class="media-card-wrapper elevation-1"
      @mouseenter="startPreview"
      @mouseleave="stopPreview"
    >
      <template v-if="isVisible">
        <video
          v-if="showHoverPreview"
          :src="computedVideoUrl"
          :type="videoType"
          class="media-card-img"
          muted
          loop
          autoplay
          playsinline
          preload="metadata"
          @error="onVideoError($event)"
        ></video>
        <img
          v-else-if="imageSrc"
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
          :src="computedVideoUrl"
          :type="videoType"
          class="media-card-img"
          muted
          playsinline
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

        <v-btn
          v-if="isViewOnly"
          variant="flat"
          class="action-btn view-only-badge"
          :title="$t('media_card.view_only')"
        >
          <v-icon size="14" color="white">mdi-cloud-outline</v-icon>
        </v-btn>

        <div v-if="isVideo" class="video-indicator">
          <v-icon color="white" size="20">mdi-play</v-icon>
        </div>

        <div v-if="isFavorite" class="favorite-heart" :class="{ pop: heartPop }">
          <v-icon color="white" size="56">mdi-heart</v-icon>
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
          v-if="isAnalyzed"
          variant="flat"
          class="action-btn ai-badge"
          :title="$t('media_card.ai_analyzed')"
        >
          <v-icon size="14" color="white">mdi-auto-fix</v-icon>
        </v-btn>
      </template>
      <div v-else class="viewport-placeholder h-100 w-100 d-flex align-center justify-center"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useMediaUrl } from '@/composables/useMediaUrl';
import { isVideo as checkIsVideo } from '@/composables/useMediaUtils';
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

const { videoUrl: buildVideoUrl, thumbUrl: buildThumbUrl, remoteThumbUrl } = useMediaUrl();

const containerRef = ref<HTMLElement | null>(null);
const isVisible = ref(false);
let observer: IntersectionObserver | null = null;

const isVideo = computed(() => {
  if (!props.path?.location) return false;
  return checkIsVideo(props.path.location);
});

const isViewOnly = computed((): boolean => !!props.path?.view_only);

const computedVideoUrl = computed(() => {
  if (!props.path?.location || !isVideo.value) return '';
  // Evicted items have no local bytes; hover preview would just 404.
  if (isViewOnly.value) return '';
  return buildVideoUrl(props.path.location) ?? '';
});

const videoType = computed(() => {
  const ext = props.path?.location?.split('.').pop()?.toLowerCase();
  if (ext === 'mp4') return 'video/mp4';
  if (ext === 'webm') return 'video/webm';
  if (ext === 'mov') return 'video/mp4';
  if (ext === 'mkv') return 'video/x-matroska';
  if (ext === 'm4v') return 'video/mp4';
  return undefined;
});

const posterFailed = ref(false);

const canHover = typeof window !== 'undefined' && window.matchMedia?.('(hover: hover)').matches;
const prefersReducedMotion =
  typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

let previewTimerId: number | undefined;
const hovering = ref(false);

function startPreview(): void {
  if (!canHover || prefersReducedMotion) return;
  if (previewTimerId !== undefined) window.clearTimeout(previewTimerId);
  previewTimerId = window.setTimeout(() => {
    hovering.value = true;
  }, 300);
}

function stopPreview(): void {
  if (previewTimerId !== undefined) {
    window.clearTimeout(previewTimerId);
    previewTimerId = undefined;
  }
  hovering.value = false;
}

onUnmounted(() => stopPreview());

// Hover-preview takes precedence over the poster; without hover support
// (touch devices) or with reduced motion the poster stays put.
const showHoverPreview = computed(
  () => isVideo.value && hovering.value && !!computedVideoUrl.value,
);

const imageSrc = computed(() => {
  if (!props.path?.location || isVideo.value) return undefined;
  // Evicted items: stream the thumbnail from the peer, fall back to the
  // inline copy that arrived with the manifest.
  if (isViewOnly.value) {
    return remoteThumbUrl(props.path.id) || props.path.encoded || undefined;
  }
  const thumb = buildThumbUrl(props.path.location);
  if (thumb) return thumb;
  return props.path.encoded || undefined;
});

// Prefer a generated poster (320px thumb from ffmpeg) over mounting a video
// element per card, which forces the browser to download media bytes.
const posterSrc = computed(() => {
  if (posterFailed.value || !isVideo.value || !props.path?.location) return undefined;
  if (isViewOnly.value) return remoteThumbUrl(props.path.id);
  return buildThumbUrl(props.path.location);
});
const notSynced = computed((): boolean => !!props.path?.sync_needed && !props.path?.received);

const NSFW_THRESHOLD = 0.8;

const nsfwScore = computed((): number => {
  if (!props.path?.properties) return 0;
  const v = props.path.properties['nsfw'];
  const score = parseFloat(String(v));
  return Number.isFinite(score) ? score : 0;
});

const isAnalyzed = computed((): boolean => {
  if (!props.path) return false;
  return (
    (props.path.objects && Object.keys(props.path.objects).length > 0) ||
    props.path.aesthetics_score != null ||
    !!props.path.caption ||
    props.path.indexed >= 2
  );
});

// Double-tap to favorite: the first tap waits TAP_DELAY before opening the
// viewer so a second tap can cancel it and toggle favorite instead.
const TAP_DELAY = 260;
let lastTap = 0;
let openTimer: number | undefined;
let popTimer: number | undefined;

const isFavorite = computed((): boolean => !!props.path?.favorite);
const heartPop = ref(false);

function triggerHeartPop(): void {
  heartPop.value = false;
  // Force a style flush so the animation restarts on consecutive likes.
  void containerRef.value?.offsetWidth;
  heartPop.value = true;
  if (popTimer !== undefined) window.clearTimeout(popTimer);
  popTimer = window.setTimeout(() => {
    heartPop.value = false;
    popTimer = undefined;
  }, 450);
}

function handleClick(): void {
  if (props.selectionMode) {
    emit('select', props.path.id);
    return;
  }
  const now = Date.now();
  if (now - lastTap < TAP_DELAY) {
    lastTap = 0;
    if (openTimer !== undefined) {
      window.clearTimeout(openTimer);
      openTimer = undefined;
    }
    emit('toggle-favorite', props.path.id);
    triggerHeartPop();
    return;
  }
  lastTap = now;
  openTimer = window.setTimeout(() => {
    openTimer = undefined;
    emit('click');
  }, TAP_DELAY);
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
  if (openTimer !== undefined) window.clearTimeout(openTimer);
  if (popTimer !== undefined) window.clearTimeout(popTimer);
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

.media-card-container:hover .action-btn {
  opacity: 1;
  transform: translateY(0);
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

.view-only-badge {
  position: absolute;
  top: 12px;
  left: 12px;
  right: auto;
  width: 28px;
  height: 28px;
  background: color-mix(in srgb, rgb(var(--v-theme-info)) 90%, transparent);
  opacity: 1;
  transform: none;
  z-index: 6;
}

.view-only-badge:hover {
  background: rgb(var(--v-theme-info));
}

.ai-badge {
  position: absolute;
  top: 12px;
  right: 12px;
  width: 28px;
  height: 28px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(8px);
  opacity: 1;
  transform: none;
  z-index: 6;
}

.ai-badge:hover {
  background: rgba(0, 0, 0, 0.7);
}

.favorite-heart {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0.55;
  pointer-events: none;
  z-index: 4;
  filter: drop-shadow(0 2px 6px rgba(0, 0, 0, 0.5));
}

.favorite-heart.pop :deep(.v-icon) {
  animation: heart-pop 0.45s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes heart-pop {
  0% {
    transform: scale(0.6);
    opacity: 0.9;
  }
  55% {
    transform: scale(1.15);
    opacity: 1;
  }
  100% {
    transform: scale(1);
    opacity: 0.55;
  }
}
</style>
