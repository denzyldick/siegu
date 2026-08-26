<template>
  <div
    class="video-player"
    @click.self="toggleControls"
    @mousemove="showControlsTemporarily"
    @mouseleave="scheduleHideControls"
  >
    <video
      ref="videoEl"
      :src="src"
      :type="type"
      class="video-element"
      playsinline
      preload="metadata"
      :muted="isMuted"
      @loadedmetadata="onMetadataLoaded"
      @timeupdate="onTimeUpdate"
      @play="onPlay"
      @pause="onPause"
      @ended="onEnded"
      @error="$emit('error', $event)"
    />

    <!-- Tap-to-play center overlay (when paused, no controls) -->
    <transition name="fade">
      <div
        v-if="!isPlaying && !controlsVisible && !hasEnded"
        class="tap-to-play"
        @click.stop="togglePlay"
      >
        <div class="tap-to-play-icon">
          <v-icon size="48" color="white">mdi-play</v-icon>
        </div>
      </div>
    </transition>

    <!-- Controls overlay -->
    <transition name="controls-fade">
      <div
        v-if="controlsVisible"
        class="controls-overlay"
        @click.stop
        @mousemove="showControlsTemporarily"
      >
        <!-- Center play/pause -->
        <button class="center-play-btn" @click.stop="togglePlay">
          <v-icon size="40" color="white">
            {{ isPlaying ? 'mdi-pause' : 'mdi-play' }}
          </v-icon>
        </button>

        <!-- Bottom controls bar -->
        <div class="controls-bar">
          <span class="time-label">{{ formatTime(currentTime) }}</span>
          <div
            class="progress-track"
            ref="progressTrackRef"
            @mousedown.prevent="startScrub"
            @touchstart.prevent="startScrub"
          >
            <div class="progress-bg" />
            <div class="progress-fill" :style="{ width: progressPercent + '%' }" />
            <div class="progress-handle" :style="{ left: progressPercent + '%' }" />
          </div>
          <span class="time-label">{{ formatTime(duration) }}</span>
          <button class="icon-btn" @click.stop="toggleMute">
            <v-icon size="20" color="white">
              {{ isMuted ? 'mdi-volume-off' : 'mdi-volume-high' }}
            </v-icon>
          </button>
        </div>
      </div>
    </transition>

    <!-- Unmute pill -->
    <transition name="pill-pop">
      <div
        v-if="isMuted && showUnmutePill && !controlsVisible"
        class="unmute-pill"
        @click.stop="unmute"
      >
        <v-icon size="14" color="white" class="mr-1">mdi-volume-high</v-icon>
        <span>Tap to unmute</span>
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue';

const props = defineProps<{
  src: string;
  type?: string;
  autoPlay?: boolean;
}>();

const emit = defineEmits<{
  play: [];
  pause: [];
  ended: [];
  error: [e: Event];
}>();

const videoEl = ref<HTMLVideoElement | null>(null);
const progressTrackRef = ref<HTMLDivElement | null>(null);

const isPlaying = ref(false);
const isMuted = ref(true);
const hasEnded = ref(false);
const currentTime = ref(0);
const duration = ref(0);
const controlsVisible = ref(true);
const showUnmutePill = ref(true);

let hideTimer: ReturnType<typeof setTimeout> | null = null;
let unmuteTimer: ReturnType<typeof setTimeout> | null = null;
let isScrubbing = false;

const progressPercent = computed(() => {
  if (duration.value === 0) return 0;
  return (currentTime.value / duration.value) * 100;
});

function onMetadataLoaded(): void {
  const video = videoEl.value;
  if (!video) return;
  duration.value = video.duration;
  if (props.autoPlay) {
    nextTick(() => {
      video.play().catch(() => {});
    });
  }
  // Hide unmute pill after 4s
  unmuteTimer = setTimeout(() => {
    showUnmutePill.value = false;
  }, 4000);
}

function onTimeUpdate(): void {
  if (isScrubbing) return;
  const video = videoEl.value;
  if (video) currentTime.value = video.currentTime;
}

function onPlay(): void {
  isPlaying.value = true;
  hasEnded.value = false;
  emit('play');
}

function onPause(): void {
  isPlaying.value = false;
  emit('pause');
}

function onEnded(): void {
  isPlaying.value = false;
  hasEnded.value = true;
  controlsVisible.value = true;
  emit('ended');
}

function togglePlay(): void {
  const video = videoEl.value;
  if (!video) return;
  if (video.paused) {
    video.play().catch(() => {});
  } else {
    video.pause();
  }
}

function toggleMute(): void {
  const video = videoEl.value;
  if (!video) return;
  isMuted.value = !isMuted.value;
  video.muted = isMuted.value;
}

function unmute(): void {
  isMuted.value = false;
  if (videoEl.value) videoEl.value.muted = false;
  showUnmutePill.value = false;
}

function showControlsTemporarily(): void {
  controlsVisible.value = true;
  scheduleHideControls();
}

function scheduleHideControls(): void {
  if (hideTimer !== null) clearTimeout(hideTimer);
  if (!isPlaying.value) return;
  hideTimer = setTimeout(() => {
    if (isPlaying.value && !isScrubbing) {
      controlsVisible.value = false;
    }
  }, 3000);
}

function toggleControls(): void {
  if (controlsVisible.value) {
    if (isPlaying.value) {
      controlsVisible.value = false;
    }
  } else {
    controlsVisible.value = true;
    scheduleHideControls();
  }
}

// Scrubbing
function startScrub(e: MouseEvent | TouchEvent): void {
  isScrubbing = true;
  updateScrub(e);
  document.addEventListener('mousemove', onScrubMove);
  document.addEventListener('mouseup', endScrub);
  document.addEventListener('touchmove', onScrubMove, { passive: false });
  document.addEventListener('touchend', endScrub);
}

function onScrubMove(e: MouseEvent | TouchEvent): void {
  e.preventDefault();
  updateScrub(e);
}

function updateScrub(e: MouseEvent | TouchEvent): void {
  const track = progressTrackRef.value;
  const video = videoEl.value;
  if (!track || !video || duration.value === 0) return;

  const rect = track.getBoundingClientRect();
  const clientX = 'touches' in e ? e.touches[0].clientX : e.clientX;
  const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
  currentTime.value = ratio * duration.value;
  video.currentTime = currentTime.value;
}

function endScrub(): void {
  isScrubbing = false;
  document.removeEventListener('mousemove', onScrubMove);
  document.removeEventListener('mouseup', endScrub);
  document.removeEventListener('touchmove', onScrubMove);
  document.removeEventListener('touchend', endScrub);
  scheduleHideControls();
}

function formatTime(seconds: number): string {
  if (!seconds || !isFinite(seconds)) return '0:00';
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, '0')}`;
}

// Expose play/pause for parent
function play(): void {
  videoEl.value?.play().catch(() => {});
}

function pause(): void {
  videoEl.value?.pause();
}

defineExpose({ play, pause });

onMounted(() => {
  showControlsTemporarily();
});

onUnmounted(() => {
  if (hideTimer !== null) clearTimeout(hideTimer);
  if (unmuteTimer !== null) clearTimeout(unmuteTimer);
  endScrub();
});
</script>

<script lang="ts">
import { computed } from 'vue';
</script>

<style scoped>
.video-player {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  user-select: none;
}

.video-element {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

/* Tap-to-play overlay */
.tap-to-play {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 5;
  cursor: pointer;
}

.tap-to-play-icon {
  width: 72px;
  height: 72px;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.tap-to-play:hover .tap-to-play-icon {
  transform: scale(1.08);
}

/* Controls overlay */
.controls-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  z-index: 10;
  pointer-events: auto;
}

.center-play-btn {
  width: 64px;
  height: 64px;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(12px);
  border: none;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition:
    transform 0.2s cubic-bezier(0.16, 1, 0.3, 1),
    background 0.2s ease;
  pointer-events: auto;
}

.center-play-btn:hover {
  transform: scale(1.08);
  background: rgba(0, 0, 0, 0.6);
}

/* Bottom controls bar */
.controls-bar {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px 16px;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.55));
  pointer-events: auto;
}

.time-label {
  font-family: 'Outfit', sans-serif;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.85);
  min-width: 36px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

/* Progress bar */
.progress-track {
  flex: 1;
  height: 20px;
  display: flex;
  align-items: center;
  cursor: pointer;
  position: relative;
}

.progress-bg {
  position: absolute;
  left: 0;
  right: 0;
  height: 3px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.25);
  transition: height 0.15s ease;
}

.progress-track:hover .progress-bg {
  height: 5px;
}

.progress-fill {
  position: absolute;
  left: 0;
  height: 3px;
  border-radius: 2px;
  background: white;
  transition: height 0.15s ease;
  pointer-events: none;
}

.progress-track:hover .progress-fill {
  height: 5px;
}

.progress-handle {
  position: absolute;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: white;
  transform: translateX(-50%) scale(0);
  transition: transform 0.15s cubic-bezier(0.16, 1, 0.3, 1);
  pointer-events: none;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
}

.progress-track:hover .progress-handle {
  transform: translateX(-50%) scale(1);
}

.icon-btn {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: transparent;
  border: none;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s ease;
}

.icon-btn:hover {
  background: rgba(255, 255, 255, 0.15);
}

/* Unmute pill */
.unmute-pill {
  position: absolute;
  top: 16px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  padding: 6px 16px;
  border-radius: 9999px;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(12px);
  color: white;
  font-family: 'Outfit', sans-serif;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  z-index: 15;
  transition:
    transform 0.2s cubic-bezier(0.16, 1, 0.3, 1),
    background 0.2s ease;
  white-space: nowrap;
}

.unmute-pill:hover {
  transform: translateX(-50%) scale(1.05);
  background: rgba(0, 0, 0, 0.7);
}

/* Transitions */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.controls-fade-enter-active,
.controls-fade-leave-active {
  transition: opacity 0.25s ease;
}
.controls-fade-enter-from,
.controls-fade-leave-to {
  opacity: 0;
}

.pill-pop-enter-active {
  transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}
.pill-pop-leave-active {
  transition: all 0.2s ease;
}
.pill-pop-enter-from {
  opacity: 0;
  transform: translateX(-50%) translateY(-8px);
}
.pill-pop-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(-4px);
}
</style>
