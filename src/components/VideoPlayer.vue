<template>
  <div
    ref="containerRef"
    class="video-player"
    tabindex="0"
    @click.self="toggleControls"
    @dblclick.self="toggleFullscreen"
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
      @progress="onProgress"
      @play="onPlay"
      @pause="onPause"
      @ended="onEnded"
      @click="videoClicked"
      @dblclick="toggleFullscreen"
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
        :class="{ 'controls-overlay--fullscreen': isFullscreen }"
        @click.stop
        @mousemove="showControlsTemporarily"
      >
        <!-- Top gradient bar -->
        <div class="controls-top">
          <span v-if="title" class="video-title">{{ title }}</span>
          <span class="spacer" />
          <v-menu
            v-model="speedMenu"
            :close-on-content-click="true"
            offset-y
            :location="'bottom end'"
          >
            <template v-slot:activator="{ props: menuProps }">
              <button
                class="icon-btn"
                v-bind="menuProps"
                :title="t('video_player.speed')"
                @click.stop="speedMenu = true"
              >
                <span class="speed-label">{{ playbackRateLabel }}</span>
              </button>
            </template>
            <v-list density="compact" class="speed-list">
              <v-list-item
                v-for="rate in speedOptions"
                :key="rate"
                :active="playbackRate === rate"
                :title="rateLabel(rate)"
                @click.stop="setPlaybackRate(rate)"
              />
            </v-list>
          </v-menu>
        </div>

        <!-- Center play/pause -->
        <button class="center-play-btn" @click.stop="togglePlay">
          <v-icon size="40" color="white">
            {{ isPlaying ? 'mdi-pause' : 'mdi-play' }}
          </v-icon>
        </button>

        <!-- Bottom controls bar -->
        <div class="controls-bar">
          <button class="icon-btn" :title="t('video_player.unmute')" @click.stop="toggleMute">
            <v-icon size="20" color="white">
              {{ isMuted ? 'mdi-volume-off' : isVolumeLow ? 'mdi-volume-medium' : 'mdi-volume-high' }}
            </v-icon>
          </button>
          <input
            class="volume-slider"
            type="range"
            min="0"
            max="1"
            step="0.05"
            :value="volume"
            :title="t('video_player.volume')"
            @input="onVolumeInput"
          />
          <span class="time-label">{{ formatTime(currentTime) }}</span>
          <div
            class="progress-track"
            ref="progressTrackRef"
            @mousedown.prevent="startScrub"
            @touchstart.prevent="startScrub"
          >
            <div class="progress-bg" />
            <div class="progress-buffered" :style="{ width: bufferedPercent + '%' }" />
            <div class="progress-fill" :style="{ width: progressPercent + '%' }" />
            <div class="progress-handle" :style="{ left: progressPercent + '%' }" />
          </div>
          <span class="time-label">
            -{{ remainingTime }} / {{ formatTime(duration) }}
          </span>
          <button
            v-if="transcript"
            class="icon-btn"
            :class="{ active: showTranscript }"
            :title="t('video_player.transcript')"
            @click.stop="toggleTranscript"
          >
            <v-icon size="20" color="white">mdi-subtitles-outline</v-icon>
          </button>
          <button
            v-if="pipSupported"
            class="icon-btn"
            :class="{ active: isPiP }"
            :title="t('video_player.pip')"
            @click.stop="togglePiP"
          >
            <v-icon size="20" color="white">mdi-picture-in-picture-bottom-right-outline</v-icon>
          </button>
          <button
            class="icon-btn"
            :title="t('video_player.fullscreen')"
            @click.stop="toggleFullscreen"
          >
            <v-icon size="20" color="white">
              {{ isFullscreen ? 'mdi-fullscreen-exit' : 'mdi-fullscreen' }}
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
        <span>{{ t('video_player.tap_to_unmute') }}</span>
      </div>
    </transition>

    <!-- Transcript overlay -->
    <transition name="fade">
      <div
        v-if="showTranscript && transcript"
        class="transcript-overlay"
        :class="{ 'transcript-overlay--fullscreen': isFullscreen }"
        @click.stop
      >
        {{ transcript }}
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, nextTick, ref } from 'vue';
import { useI18n } from 'vue-i18n';

const props = defineProps<{
  src: string;
  type?: string;
  autoPlay?: boolean;
  title?: string;
  transcript?: string;
}>();

const emit = defineEmits<{
  play: [];
  pause: [];
  ended: [];
  error: [e: Event];
}>();

const { t } = useI18n();

const containerRef = ref<HTMLDivElement | null>(null);
const videoEl = ref<HTMLVideoElement | null>(null);
const progressTrackRef = ref<HTMLDivElement | null>(null);

const isPlaying = ref(false);
const isMuted = ref(true);
const hasEnded = ref(false);
const currentTime = ref(0);
const duration = ref(0);
const bufferedEnd = ref(0);
const volume = ref(1);
const playbackRate = ref(1);
const controlsVisible = ref(true);
const showUnmutePill = ref(true);
const showTranscript = ref(false);
const isFullscreen = ref(false);
const isPiP = ref(false);
const speedMenu = ref(false);

const speedOptions = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];

let hideTimer: ReturnType<typeof setTimeout> | null = null;
let unmuteTimer: ReturnType<typeof setTimeout> | null = null;
let isScrubbing = false;
let pipSupported = false;

const progressPercent = computed(() => {
  if (duration.value === 0) return 0;
  return (currentTime.value / duration.value) * 100;
});

const bufferedPercent = computed(() => {
  if (duration.value === 0) return 0;
  return Math.max(0, Math.min(100, (bufferedEnd.value / duration.value) * 100));
});

const remainingTime = computed(() =>
  Math.max(0, duration.value - currentTime.value).toFixed(0),
);

const playbackRateLabel = computed(() => rateLabel(playbackRate.value));

const isVolumeLow = computed(() => volume.value > 0 && volume.value < 0.5);

function rateLabel(rate: number): string {
  return `${rate}x`;
}

function onMetadataLoaded(): void {
  const video = videoEl.value;
  if (!video) return;
  duration.value = video.duration;
  if (props.autoPlay) {
    nextTick(() => {
      video.play().catch(() => {});
    });
  }
  unmuteTimer = setTimeout(() => {
    showUnmutePill.value = false;
  }, 4000);
}

function onTimeUpdate(): void {
  if (isScrubbing) return;
  const video = videoEl.value;
  if (video) currentTime.value = video.currentTime;
}

function onProgress(): void {
  const video = videoEl.value;
  if (!video || !video.buffered.length) return;
  bufferedEnd.value = video.buffered.end(video.buffered.length - 1);
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

function videoClicked(): void {
  const video = videoEl.value;
  video?.focus({ preventScroll: true });
  if (controlsVisible.value) {
    togglePlay();
  } else {
    showControlsTemporarily();
  }
  showControlsTemporarily();
}

function toggleMute(): void {
  const video = videoEl.value;
  if (!video) return;
  if (video.muted) {
    if (volume.value === 0) volume.value = 1;
  }
  isMuted.value = !video.muted;
  video.muted = isMuted.value;
  showControlsTemporarily();
}

function onVolumeInput(e: Event): void {
  const slider = e.target as HTMLInputElement;
  const next = Number(slider.value);
  volume.value = next;
  const video = videoEl.value;
  if (!video) return;
  video.volume = next;
  if (next === 0) {
    video.muted = true;
    isMuted.value = true;
  } else if (video.muted) {
    video.muted = false;
    isMuted.value = false;
  }
  showControlsTemporarily();
}

function unmute(): void {
  const video = videoEl.value;
  isMuted.value = false;
  if (video) {
    video.muted = false;
    video.volume = volume.value || 1;
  }
  volume.value = video?.volume || volume.value || 1;
  showUnmutePill.value = false;
}

function setPlaybackRate(rate: number): void {
  playbackRate.value = rate;
  const video = videoEl.value;
  if (video) video.playbackRate = rate;
  speedMenu.value = false;
}

function toggleTranscript(): void {
  showTranscript.value = !showTranscript.value;
  showControlsTemporarily();
}

async function toggleFullscreen(): Promise<void> {
  const container = containerRef.value;
  if (!container) return;
  try {
    if (!document.fullscreenElement) {
      await container.requestFullscreen();
    } else {
      await document.exitFullscreen();
    }
  } catch {
    // Fullscreen may be blocked by the browser; ignore silently.
  }
}

async function togglePiP(): Promise<void> {
  const video = videoEl.value;
  if (!video) return;
  try {
    if (document.pictureInPictureElement) {
      await document.exitPictureInPicture();
    } else {
      await video.requestPictureInPicture();
    }
  } catch {
    // PiP may be unavailable (e.g. inside fullscreen); ignore silently.
  }
}

function handleWindowKeydown(e: KeyboardEvent): void {
  if (!videoEl.value) return;
  const target = e.target as HTMLElement | null;
  const editable =
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    (target?.isContentEditable ?? false);
  if (editable) return;

  if (e.key === 'Escape' && isFullscreen.value) {
    e.stopImmediatePropagation();
    void document.exitFullscreen();
    return;
  }

  if (isScrubbing) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      endScrub();
    }
    return;
  }

  const gateActive = target === videoEl.value || isFullscreen.value;
  const seek = (delta: number): void => {
    const video = videoEl.value;
    if (!video) return;
    video.currentTime = Math.max(0, Math.min(video.duration || 0, video.currentTime + delta));
    currentTime.value = video.currentTime;
    showControlsTemporarily();
  };

  switch (e.key) {
    case ' ':
      e.preventDefault();
      togglePlay();
      break;
    case 'k':
      togglePlay();
      break;
    case 'm':
      toggleMute();
      break;
    case 'f':
      e.preventDefault();
      void toggleFullscreen();
      break;
    case 'p':
      if (pipSupported) void togglePiP();
      break;
    case 'ArrowLeft':
      if (gateActive) {
        e.preventDefault();
        e.stopImmediatePropagation();
        seek(e.shiftKey ? -30 : -5);
      }
      break;
    case 'ArrowRight':
      if (gateActive) {
        e.preventDefault();
        e.stopImmediatePropagation();
        seek(e.shiftKey ? 30 : 5);
      }
      break;
    case 'ArrowUp':
      if (gateActive) {
        e.preventDefault();
        e.stopImmediatePropagation();
        adjustVolume(0.1);
      }
      break;
    case 'ArrowDown':
      if (gateActive) {
        e.preventDefault();
        e.stopImmediatePropagation();
        adjustVolume(-0.1);
      }
      break;
    default:
      if (e.key >= '0' && e.key <= '9' && gateActive) {
        e.preventDefault();
        e.stopImmediatePropagation();
        const video = videoEl.value;
        if (video && video.duration > 0) {
          video.currentTime = (Number(e.key) / 10) * video.duration;
          currentTime.value = video.currentTime;
          showControlsTemporarily();
        }
      }
  }
}

function adjustVolume(delta: number): void {
  const video = videoEl.value;
  if (!video) return;
  volume.value = Math.max(0, Math.min(1, volume.value + delta));
  video.volume = volume.value;
  if (volume.value > 0 && video.muted) {
    video.muted = false;
    isMuted.value = false;
  }
  showControlsTemporarily();
}

function showControlsTemporarily(): void {
  controlsVisible.value = true;
  scheduleHideControls();
}

function scheduleHideControls(): void {
  if (hideTimer !== null) clearTimeout(hideTimer);
  if (!isPlaying.value || isPiP.value) return;
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

function onFullscreenChange(): void {
  isFullscreen.value = document.fullscreenElement === containerRef.value;
}

function onPiPChange(): void {
  isPiP.value = document.pictureInPictureElement === videoEl.value;
  if (isPiP.value) {
    controlsVisible.value = false;
  }
}

onMounted(() => {
  pipSupported = typeof document !== 'undefined' && document.pictureInPictureEnabled;
  const video = videoEl.value;
  if (video) video.volume = volume.value;
  showControlsTemporarily();
  document.addEventListener('keydown', handleWindowKeydown);
  document.addEventListener('fullscreenchange', onFullscreenChange);
  document.addEventListener('enterpictureinpicture', onPiPChange);
  document.addEventListener('leavepictureinpicture', onPiPChange);
});

onUnmounted(() => {
  if (hideTimer !== null) clearTimeout(hideTimer);
  if (unmuteTimer !== null) clearTimeout(unmuteTimer);
  endScrub();
  document.removeEventListener('keydown', handleWindowKeydown);
  document.removeEventListener('fullscreenchange', onFullscreenChange);
  document.removeEventListener('enterpictureinpicture', onPiPChange);
  document.removeEventListener('leavepictureinpicture', onPiPChange);
  if (document.fullscreenElement === containerRef.value) {
    void document.exitFullscreen();
  }
});
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
  background: #000;
  outline: none;
  cursor: default;
}

.video-element {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  cursor: pointer;
}

.video-player:fullscreen .video-element {
  width: 100%;
  height: 100%;
}

.video-player:fullscreen {
  cursor: default;
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
  background: linear-gradient(
    180deg,
    rgba(0, 0, 0, 0.35) 0%,
    transparent 25%,
    transparent 75%,
    rgba(0, 0, 0, 0.55) 100%
  );
}

.controls-overlay--fullscreen {
  background: rgba(0, 0, 0, 0.25);
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

/* Top gradient bar */
.controls-top {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  background: linear-gradient(rgba(0, 0, 0, 0.55), transparent);
}

.video-title {
  font-family: 'Outfit', sans-serif;
  font-size: 13px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.9);
  text-shadow: 0 1px 6px rgba(0, 0, 0, 0.5);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spacer {
  flex: 1;
}

.speed-label {
  font-family: 'Outfit', sans-serif;
  font-size: 13px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
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
  padding: 10px 16px 12px;
  pointer-events: auto;
}

.time-label {
  font-family: 'Outfit', sans-serif;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.85);
  min-width: 44px;
  text-align: center;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* Volume slider */
.volume-slider {
  width: 70px;
  height: 4px;
  accent-color: #fff;
  cursor: pointer;
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

.progress-buffered {
  position: absolute;
  left: 0;
  height: 3px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.35);
  pointer-events: none;
  transition: height 0.15s ease;
}

.progress-track:hover .progress-buffered {
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
  width: 34px;
  height: 34px;
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

.icon-btn.active {
  background: rgba(255, 255, 255, 0.25);
}

/* Speed menu list */
.speed-list {
  min-width: 140px;
}

/* Transcript overlay */
.transcript-overlay {
  position: absolute;
  left: 8%;
  right: 8%;
  bottom: 64px;
  padding: 10px 16px;
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(8px);
  color: white;
  font-family: 'Outfit', sans-serif;
  font-size: 13px;
  line-height: 1.5;
  text-align: center;
  z-index: 12;
  pointer-events: auto;
  max-height: 40%;
  overflow-y: auto;
  white-space: pre-wrap;
}

.transcript-overlay--fullscreen {
  bottom: 80px;
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