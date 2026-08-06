<template>
  <v-dialog v-model="visible" fullscreen transition="dialog-bottom-transition">
    <v-card rounded="0" color="background" class="fill-height" style="overflow: hidden">
      <v-layout class="fill-height">
        <v-main
          class="fill-height position-relative d-flex flex-column align-center justify-center p-0"
          style="background-color: rgb(var(--v-theme-background))"
        >
          <v-btn
            icon="mdi-close"
            variant="text"
            color="#18181b"
            class="viewer-nav-btn top-left"
            @click="close"
          ></v-btn>
          <template v-if="isMobile">
            <v-btn
              icon="mdi-dots-vertical"
              variant="text"
              color="#71717a"
              class="viewer-nav-btn top-left-more"
              @click="moreMenuOpen = true"
            ></v-btn>
            <v-bottom-sheet v-model="moreMenuOpen">
              <v-list density="compact" class="siegu-list">
                <v-list-item
                  v-for="item in moreItems"
                  :key="item.key"
                  @click="closeMoreMenu(item.action)"
                  :prepend-icon="item.icon"
                >
                  <v-list-item-title>{{ $t('media_viewer.' + item.key) }}</v-list-item-title>
                </v-list-item>
              </v-list>
            </v-bottom-sheet>
          </template>
          <v-menu v-else location="bottom start" close-on-content-click>
            <template v-slot:activator="{ props: menuProps }">
              <v-btn
                v-bind="menuProps"
                icon="mdi-dots-vertical"
                variant="text"
                color="#71717a"
                class="viewer-nav-btn top-left-more"
              ></v-btn>
            </template>
            <v-list density="compact" class="siegu-list">
              <v-list-item
                v-for="item in moreItems"
                :key="item.key"
                @click="item.action()"
                :prepend-icon="item.icon"
              >
                <v-list-item-title>{{ $t('media_viewer.' + item.key) }}</v-list-item-title>
              </v-list-item>
            </v-list>
          </v-menu>
          <v-btn
            v-if="!isVideo && !showInfo"
            icon="mdi-information-outline"
            variant="text"
            color="#71717a"
            class="viewer-nav-btn top-right"
            @click="showInfo = !showInfo"
          ></v-btn>

          <div
            class="touch-overlay"
            v-touch="{
              left: () => next(),
              right: () => prev(),
              down: () => close(),
            }"
          ></div>

          <div class="viewer-content-container">
            <v-btn
              v-if="!isMobile"
              icon="mdi-chevron-left"
              variant="text"
              color="#18181b"
              size="x-large"
              @click="prev"
              class="side-nav-btn left"
            ></v-btn>

            <div class="media-wrapper">
              <img
                v-if="currentPhoto && !isVideo"
                :src="currentPhotoSrc"
                class="viewer-image"
                decoding="async"
              />
              <video
                v-if="currentPhoto && isVideo && computedVideoUrl"
                ref="videoPlayer"
                :src="computedVideoUrl"
                class="viewer-image"
                controls
                preload="metadata"
                @error="onVideoError"
              ></video>
            </div>

            <v-btn
              v-if="!isMobile"
              icon="mdi-chevron-right"
              variant="text"
              color="#18181b"
              size="x-large"
              @click="next"
              class="side-nav-btn right"
            ></v-btn>
          </div>

          <div class="thumbnail-rail-container">
            <RecycleScroller
              ref="scrollerRef"
              class="thumbnail-rail"
              :items="photos"
              :item-size="68"
              direction="horizontal"
              v-slot="{ item, index: railIndex }"
            >
              <MediaThumbnail
                :photo="item"
                :active="railIndex === index"
                @click="$emit('update:index', railIndex)"
              />
            </RecycleScroller>
          </div>
        </v-main>

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
            <v-toolbar-title class="text-zinc-primary text-subtitle-1 font-weight-bold">{{
              $t('media_viewer.metadata')
            }}</v-toolbar-title>
          </v-toolbar>

          <v-divider class="opacity-5"></v-divider>

          <v-list class="bg-transparent px-4">
            <div class="mb-4" v-if="currentPhoto && currentPhoto.indexed < 2">
              <v-btn
                block
                variant="flat"
                color="primary"
                prepend-icon="mdi-auto-fix"
                :loading="isAnalyzing"
                @click="analyzePhoto"
                class="text-none"
              >
                {{ $t('media_viewer.analyze') }}
              </v-btn>
              <div v-if="isAnalyzing" class="text-caption text-zinc-muted mt-2 text-center">
                <span class="analyzing-dots">{{ $t('media_viewer.analyzing') }}</span>
              </div>
              <div v-else-if="globalEta" class="text-caption text-zinc-muted mt-2 text-center">
                {{ $t('media_viewer.library_indexing', { time: formatEta(globalEta) }) }}
              </div>
            </div>

            <v-divider class="opacity-5 mb-4" v-if="modelChips.length > 0"></v-divider>

            <div class="mb-6" v-if="modelChips.length > 0">
              <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">
                {{ $t('media_viewer.run_model') }}
              </div>
              <div v-if="isAnalyzingModel" class="d-flex align-center mb-3">
                <v-progress-circular
                  indeterminate
                  size="16"
                  width="2"
                  color="black"
                  class="mr-2"
                ></v-progress-circular>
                <span class="text-body-2 text-zinc-primary font-weight-bold">
                  {{
                    $t('media_viewer.running_model', {
                      model: $t('models.' + isAnalyzingModel + '.title'),
                    })
                  }}
                  <span class="text-caption text-zinc-muted ml-1"
                    >({{ formatElapsed(runStartTime, runTimerTick) }})</span
                  >
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
              <div class="text-caption text-zinc-muted mb-1 text-uppercase tracking-widest">
                {{ $t('media_viewer.file_details') }}
              </div>
              <div class="d-flex align-start mb-2">
                <v-icon size="small" color="#71717a" class="mr-2 mt-1"
                  >mdi-file-document-outline</v-icon
                >
                <div class="text-body-2 text-zinc-secondary word-break-all">
                  {{ currentPhoto?.location }}
                </div>
              </div>
            </div>

            <v-divider class="opacity-5 mb-4"></v-divider>

            <div class="mb-6" v-if="currentPhoto?.caption">
              <div class="text-caption text-zinc-muted mb-1 text-uppercase tracking-widest">
                {{ $t('media_viewer.ai_caption') }}
              </div>
              <div class="text-body-2 text-zinc-primary font-italic">
                "{{ currentPhoto.caption }}"
              </div>
            </div>

            <v-divider class="opacity-5 mb-4" v-if="currentPhoto?.caption"></v-divider>

            <div class="mb-6" v-if="photoOcr && !ocrLoading">
              <div class="text-caption text-zinc-muted mb-1 text-uppercase tracking-widest">
                {{ $t('media_viewer.recognized_text') }}
              </div>
              <div class="text-body-2 text-zinc-secondary ocr-text">{{ photoOcr }}</div>
              <v-btn
                size="x-small"
                variant="text"
                class="mt-1 text-none"
                :title="$t('media_viewer.copy_text')"
                @click="copyOcrText"
              >
                <v-icon size="14" class="mr-1">mdi-content-copy</v-icon>
                {{ $t('media_viewer.copy') }}
              </v-btn>
            </div>

            <v-divider class="opacity-5 mb-4" v-if="photoOcr && !ocrLoading && hasExif"></v-divider>

            <div class="mb-6" v-if="hasExif">
              <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">
                {{ $t('media_viewer.camera_settings') }}
              </div>

              <div class="d-flex align-center mb-4" v-if="exifData.make || exifData.model">
                <v-icon size="small" color="#71717a" class="mr-2">mdi-camera</v-icon>
                <span class="text-body-2 text-zinc-secondary"
                  >{{ exifData.make }} {{ exifData.model }}</span
                >
              </div>

              <v-row dense>
                <v-col cols="6" v-if="exifData.date" class="mb-3">
                  <div class="text-caption text-zinc-muted">
                    {{ $t('media_viewer.date_taken') }}
                  </div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.date }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.dimensions" class="mb-3">
                  <div class="text-caption text-zinc-muted">
                    {{ $t('media_viewer.resolution') }}
                  </div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.dimensions }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.iso" class="mb-3">
                  <div class="text-caption text-zinc-muted">{{ $t('media_viewer.iso') }}</div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.iso }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.shutter" class="mb-3">
                  <div class="text-caption text-zinc-muted">{{ $t('media_viewer.shutter') }}</div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.shutter }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.aperture" class="mb-3">
                  <div class="text-caption text-zinc-muted">{{ $t('media_viewer.aperture') }}</div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.aperture }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.focalLength" class="mb-3">
                  <div class="text-caption text-zinc-muted">
                    {{ $t('media_viewer.focal_length') }}
                  </div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.focalLength }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.lens" class="mb-3">
                  <div class="text-caption text-zinc-muted">{{ $t('media_viewer.lens') }}</div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.lens }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.flash" class="mb-3">
                  <div class="text-caption text-zinc-muted">{{ $t('media_viewer.flash') }}</div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.flash }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.whiteBalance" class="mb-3">
                  <div class="text-caption text-zinc-muted">
                    {{ $t('media_viewer.white_balance') }}
                  </div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.whiteBalance }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.meteringMode" class="mb-3">
                  <div class="text-caption text-zinc-muted">
                    {{ $t('media_viewer.metering_mode') }}
                  </div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.meteringMode }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.software" class="mb-3">
                  <div class="text-caption text-zinc-muted">{{ $t('media_viewer.software') }}</div>
                  <div class="text-body-2 text-zinc-secondary">{{ exifData.software }}</div>
                </v-col>
              </v-row>
            </div>

            <v-divider class="opacity-5 mb-4"></v-divider>

            <div class="mb-6">
              <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">
                {{ $t('media_viewer.people_in_photo') }}
              </div>
              <div
                v-if="detectedFaces.length === 0"
                class="text-body-2 text-zinc-muted font-italic"
              >
                {{ $t('media_viewer.no_faces') }}
              </div>
              <div class="d-flex flex-wrap ga-3">
                <div
                  v-for="face in uniquePeople"
                  :key="String(face.face_id)"
                  class="d-flex flex-column align-center cursor-pointer"
                  @click="goToPerson(face)"
                  style="width: 70px"
                >
                  <v-avatar size="56" class="border-subtle mb-1">
                    <v-img :src="face.encoded ?? ''" cover></v-img>
                  </v-avatar>
                  <div
                    class="text-caption text-zinc-primary text-truncate text-center w-100 font-weight-bold"
                  >
                    {{ face.person_name || $t('media_viewer.unnamed') }}
                  </div>
                </div>
              </div>
            </div>

            <v-divider class="opacity-5 mb-4"></v-divider>

            <div class="mb-6">
              <div class="text-caption text-zinc-muted mb-3 text-uppercase tracking-widest">
                {{ $t('media_viewer.ai_insights') }}
              </div>

              <div v-if="aiTags.length === 0" class="text-body-2 text-zinc-muted font-italic">
                {{ $t('media_viewer.no_insights') }}
              </div>

              <div v-for="tag in aiTags" :key="tag.name" class="mb-4">
                <div class="d-flex align-center justify-space-between w-100">
                  <span class="text-body-2 text-zinc-secondary text-capitalize">{{
                    tag.name
                  }}</span>
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
          <v-icon size="small" class="mr-3" :color="snackbar.error ? 'error' : 'white'">{{
            snackbar.error ? 'mdi-alert-circle' : 'mdi-check-circle'
          }}</v-icon>
          <span class="text-body-2">{{ snackbar.text }}</span>
        </div>
      </v-snackbar>
      <AddToAlbumSheet
        v-model="addToAlbumOpen"
        :photo-ids="addToAlbumPhotoIds"
        @added="onAddedToAlbum"
      />
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { revealItemInDir, openPath } from '@tauri-apps/plugin-opener';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import MediaThumbnail from './MediaThumbnail.vue';
import { RecycleScroller } from 'vue-virtual-scroller';
import AddToAlbumSheet from '@/components/albums/AddToAlbumSheet.vue';
import { isVideo as checkIsVideo } from '@/composables/useMediaUtils';
import { useMediaUrl } from '@/composables/useMediaUrl';
import { useI18n } from 'vue-i18n';
import type { MediaItem } from '@/types/media';

const { t } = useI18n();
const { ensurePort, videoUrl: buildVideoUrl } = useMediaUrl();

interface DetectedFace {
  photo_id: string;
  face_id: string;
  crop_path: string;
  encoded: string | null;
  person_id: string | null;
  person_name: string | null;
}

const props = defineProps<{
  modelValue: boolean;
  photos: MediaItem[];
  index: number;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
  'update:index': [index: number];
  'update:photo': [photo: MediaItem];
  'navigate-to-person': [person: { id: string; name: string }];
}>();

const showInfo = ref(false);
const os = ref('');
const moreMenuOpen = ref(false);
const addToAlbumOpen = ref(false);
const detectedFaces = ref<DetectedFace[]>([]);
const photoOcr = ref('');
const ocrLoading = ref(false);
const isAnalyzing = ref(false);
const isAnalyzingModel = ref<string | null>(null);
const runStartTime = ref(0);
const runTimerTick = ref(0);
let runTimer: ReturnType<typeof setInterval> | null = null;
const globalEta = ref(0);
let unlistenEta: UnlistenFn | null = null;
let unlistenResult: UnlistenFn | null = null;
const snackbar = ref({ show: false, text: '', error: false });
const downloadedModels = ref<string[]>([]);
const videoPlayer = ref<HTMLVideoElement | null>(null);
const scrollerRef = ref<{
  scrollToItem: (index: number, options?: ScrollToOptions) => void;
} | null>(null);

const modelInfo = [
  { id: 'clip' },
  { id: 'face' },
  { id: 'ocr' },
  { id: 'nsfw' },
  { id: 'aesthetics' },
  { id: 'yolo' },
  { id: 'blip' },
  { id: 'midas' },
  { id: 'whisper' },
];

const isMobile = computed(() => os.value === 'android' || os.value === 'ios');

const visible = computed({
  get: () => props.modelValue,
  set: (val: boolean) => emit('update:modelValue', val),
});

const currentPhoto = computed(() => {
  if (!props.photos || props.photos.length === 0) return null;
  return props.photos[props.index];
});

const isVideo = computed(() => {
  if (!currentPhoto.value?.location) return false;
  return checkIsVideo(currentPhoto.value.location);
});

const computedVideoUrl = computed(() => {
  if (!currentPhoto.value || !isVideo.value) return '';
  return buildVideoUrl(currentPhoto.value.location) ?? '';
});

const currentPhotoSrc = computed(() => {
  if (!currentPhoto.value || isVideo.value) return '';
  const ext = currentPhoto.value.location.split('.').pop()?.toLowerCase();
  if (['heic', 'heif'].includes(ext ?? '')) {
    return currentPhoto.value.encoded || convertFileSrc(currentPhoto.value.location);
  }
  return convertFileSrc(currentPhoto.value.location);
});

interface ExifData {
  make?: string;
  model?: string;
  date?: string;
  dimensions?: string;
  iso?: string;
  shutter?: string;
  aperture?: string;
  lens?: string;
  lensMake?: string;
  focalLength?: string;
  focalLength35?: string;
  flash?: string;
  whiteBalance?: string;
  exposureProgram?: string;
  meteringMode?: string;
  sceneType?: string;
  software?: string;
}

const exifData = computed((): ExifData => {
  if (!currentPhoto.value?.properties) return {} as ExifData;
  const props = currentPhoto.value.properties;
  let dimensions: string | null = null;
  if (props['PixelXDimension'] && props['PixelYDimension']) {
    dimensions = `${props['PixelXDimension']} x ${props['PixelYDimension']}`;
  } else if (props['ImageWidth'] && props['ImageLength']) {
    dimensions = `${props['ImageWidth']} x ${props['ImageLength']}`;
  }
  return {
    make: props['Make'] as string | undefined,
    model: props['Model'] as string | undefined,
    date: (props['DateTimeOriginal'] || props['DateTime']) as string | undefined,
    dimensions: dimensions ?? undefined,
    iso: props['PhotographicSensitivity'] as string | undefined,
    shutter: props['ExposureTime'] as string | undefined,
    aperture: props['FNumber'] as string | undefined,
    lens: props['LensModel'] as string | undefined,
    lensMake: props['LensMake'] as string | undefined,
    focalLength: props['FocalLength'] as string | undefined,
    focalLength35: props['FocalLengthIn35mmFilm'] as string | undefined,
    flash: props['Flash'] as string | undefined,
    whiteBalance: props['WhiteBalance'] as string | undefined,
    exposureProgram: props['ExposureProgram'] as string | undefined,
    meteringMode: props['MeteringMode'] as string | undefined,
    sceneType: props['SceneCaptureType'] as string | undefined,
    software: props['Software'] as string | undefined,
  };
});

const hasExif = computed(() =>
  Object.values(exifData.value).some((val) => val !== undefined && val !== null),
);

const aiTags = computed(() => {
  if (!currentPhoto.value?.objects) return [];
  return Object.entries(currentPhoto.value.objects)
    .map(([name, score]) => ({ name, percent: Math.round(score * 100) }))
    .sort((a, b) => b.percent - a.percent);
});

const uniquePeople = computed(() => {
  if (!detectedFaces.value) return [];
  const seen = new Set();
  return detectedFaces.value.filter((face: DetectedFace) => {
    if (!face.person_id) return true;
    if (seen.has(face.person_id as string)) return false;
    seen.add(face.person_id as string);
    return true;
  });
});

const modelChips = computed(() => {
  if (!currentPhoto.value) return [];
  const status = (currentPhoto.value.ai_status ?? {}) as Record<string, number>;
  return modelInfo
    .filter((m) => downloadedModels.value.includes(m.id))
    .map((m) => ({
      id: m.id,
      done: status[m.id] === 1,
    }));
});

function clearAnalysisListener(): void {
  if (unlistenResult) {
    unlistenResult();
    unlistenResult = null;
  }
}

function onVideoError(): void {
  snackbar.value = { show: true, text: 'Failed to load video', error: true };
}

function stopVideo(): void {
  const video = videoPlayer.value;
  if (video) {
    video.pause();
    video.removeAttribute('src');
    video.load();
  }
}

async function fetchFaces(): Promise<void> {
  if (!currentPhoto.value) return;
  try {
    const facesStr = await invoke<string>('get_faces_for_photo', {
      photoId: currentPhoto.value.id,
    });
    detectedFaces.value = JSON.parse(facesStr);
  } catch (e) {
    console.error('Failed to fetch faces', e);
  }
}

async function loadOcr(): Promise<void> {
  if (!currentPhoto.value || isVideo.value) {
    photoOcr.value = '';
    return;
  }
  ocrLoading.value = true;
  try {
    photoOcr.value = await invoke<string>('get_photo_ocr', { id: currentPhoto.value.id });
  } catch (e) {
    console.error('Failed to fetch OCR text', e);
    photoOcr.value = '';
  } finally {
    ocrLoading.value = false;
  }
}

async function copyOcrText(): Promise<void> {
  if (!photoOcr.value) return;
  try {
    await navigator.clipboard.writeText(photoOcr.value);
    snackbar.value = { show: true, text: t('media_viewer.copied'), error: false };
  } catch {
    snackbar.value = { show: true, text: t('media_viewer.copy_failed'), error: true };
  }
}

function goToPerson(face: DetectedFace): void {
  if (!face.person_id) return;
  emit('navigate-to-person', {
    id: face.person_id as string,
    name: face.person_name || 'Unnamed',
  });
  close();
}

async function analyzePhoto(): Promise<void> {
  if (!currentPhoto.value || isAnalyzing.value || isAnalyzingModel.value) return;
  isAnalyzing.value = true;
  const photoId = currentPhoto.value.id;
  const startTime = Date.now();
  try {
    clearAnalysisListener();
    const unlisten = await listen<{
      id: string | number;
      object_count?: number;
      face_count?: number;
      has_caption?: boolean;
    }>('photo-analysis-result', (event) => {
      if (String(event.payload.id) === String(photoId)) {
        isAnalyzing.value = false;
        fetchFaces();
        unlisten();
        if (unlistenResult === unlisten) unlistenResult = null;

        const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);
        const r = event.payload;
        const parts: string[] = [];
        if (r.object_count && r.object_count > 0) parts.push(`${r.object_count} objects`);
        if (r.face_count && r.face_count > 0) parts.push(`${r.face_count} faces`);
        if (r.has_caption) parts.push('caption');
        if (parts.length === 0) parts.push('nothing detected');

        snackbar.value.text = `Analysis complete: ${parts.join(', ')} (${elapsed}s)`;
        snackbar.value.show = true;
        refreshPhoto(photoId);
        showInfo.value = true;
      }
    });
    unlistenResult = unlisten;
    await invoke('analyze_photo', { id: photoId });
  } catch (e) {
    console.error('Analysis failed', e);
    isAnalyzing.value = false;
    clearAnalysisListener();
  }
}

async function runSingleModel(modelId: string): Promise<void> {
  if (!currentPhoto.value || isAnalyzing.value || isAnalyzingModel.value) return;
  isAnalyzingModel.value = modelId;
  runStartTime.value = Date.now();
  runTimerTick.value = 0;
  runTimer = window.setInterval(() => {
    runTimerTick.value += 1;
  }, 1000);
  const photoId = currentPhoto.value.id;
  try {
    clearAnalysisListener();
    const unlisten = await listen<{ id: string | number; model_timings?: Record<string, number> }>(
      'photo-analysis-result',
      (event) => {
        if (String(event.payload.id) === String(photoId)) {
          isAnalyzingModel.value = null;
          if (runTimer) {
            clearInterval(runTimer);
            runTimer = null;
          }
          fetchFaces();
          unlisten();
          if (unlistenResult === unlisten) unlistenResult = null;
          refreshPhoto(photoId);
          showInfo.value = true;
          const modelTimings = event.payload.model_timings ?? {};
          const modelTime = modelTimings[modelId];
          const elapsed = (modelTime ?? (Date.now() - runStartTime.value) / 1000).toFixed(1);
          snackbar.value.text = `${modelId} complete (${elapsed}s)`;
          snackbar.value.error = false;
          snackbar.value.show = true;
        }
      },
    );
    unlistenResult = unlisten;
    await invoke('analyze_photo_model', { id: photoId, modelId });
  } catch (e) {
    console.error('Model analysis failed', e);
    isAnalyzingModel.value = null;
    if (runTimer) {
      clearInterval(runTimer);
      runTimer = null;
    }
    snackbar.value.text = `${modelId} failed`;
    snackbar.value.error = true;
    snackbar.value.show = true;
    clearAnalysisListener();
  }
}

async function refreshPhoto(photoId: string | number): Promise<void> {
  try {
    const photoJson = await invoke<string>('get_photo_by_id', { id: photoId });
    if (!photoJson || photoJson === 'null') return;
    const updated = JSON.parse(photoJson) as MediaItem;
    const idx = props.photos.findIndex((p) => p.id === photoId);
    if (idx !== -1) emit('update:photo', updated);
  } catch (e) {
    console.error('Failed to refresh photo', e);
  }
}

function formatEta(ms: number): string {
  if (!ms || ms < 0) return '';
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${totalSeconds % 60}s`;
}

async function listenForEta(): Promise<void> {
  unlistenEta = await listen<number>('indexing-eta', (event) => {
    globalEta.value = event.payload as number;
  });
}

function close(): void {
  stopVideo();
  visible.value = false;
}

async function handleSetWallpaper(): Promise<void> {
  if (!currentPhoto.value) return;
  try {
    await invoke('set_wallpaper', { path: currentPhoto.value.location });
    snackbar.value = { show: true, text: 'Wallpaper set', error: false };
  } catch {
    snackbar.value = { show: true, text: 'Failed to set wallpaper', error: true };
  }
}

async function handleShowInExplorer(): Promise<void> {
  if (!currentPhoto.value) return;
  try {
    await revealItemInDir(currentPhoto.value.location);
    snackbar.value = { show: true, text: 'Opened in explorer', error: false };
  } catch {
    snackbar.value = { show: true, text: 'Failed to open explorer', error: true };
  }
}

async function handleOpenWith(): Promise<void> {
  if (!currentPhoto.value) return;
  try {
    await openPath(currentPhoto.value.location);
  } catch (e) {
    console.error('Failed to open with default app', e);
  }
}

const addToAlbumPhotoIds = computed(() => {
  if (!currentPhoto.value) return [];
  return [String(currentPhoto.value.id)];
});

function onAddedToAlbum(albumName: string): void {
  snackbar.value = {
    show: true,
    error: false,
    text: t('albums.added_to_album', { album: albumName }),
  };
}

const moreItems = computed(() => {
  const items: Array<{ key: string; icon: string; action: () => void }> = [
    {
      key: 'set_wallpaper',
      icon: 'mdi-wallpaper',
      action: handleSetWallpaper,
    },
    {
      key: 'show_in_explorer',
      icon: 'mdi-folder-open-outline',
      action: handleShowInExplorer,
    },
    {
      key: 'open_with_app',
      icon: 'mdi-open-in-new',
      action: handleOpenWith,
    },
    {
      key: 'add_to_album',
      icon: 'mdi-image-plus',
      action: () => {
        moreMenuOpen.value = false;
        addToAlbumOpen.value = true;
      },
    },
  ];
  return items.filter((item) => {
    if (item.key === 'set_wallpaper' && os.value === 'ios') return false;
    if (item.key === 'show_in_explorer' && isMobile.value) return false;
    return true;
  });
});

function closeMoreMenu(action: () => void): void {
  moreMenuOpen.value = false;
  action();
}

function formatElapsed(start: number, tick: number): string {
  if (!start) return '0s';
  void tick;
  const sec = Math.floor((Date.now() - start) / 1000);
  if (sec < 60) return `${sec}s`;
  const m = Math.floor(sec / 60);
  return `${m}m ${sec % 60}s`;
}

function next(): void {
  if (props.photos.length === 0) return;
  emit('update:index', (props.index + 1) % props.photos.length);
}

function prev(): void {
  if (props.photos.length === 0) return;
  emit('update:index', (props.index - 1 + props.photos.length) % props.photos.length);
}

function handleKeydown(e: KeyboardEvent): void {
  if (!visible.value) return;
  if (e.key === 'ArrowRight') next();
  if (e.key === 'ArrowLeft') prev();
  if (e.key === 'Escape') close();
  if (e.key === 'i') showInfo.value = !showInfo.value;
}

function scrollToActiveThumb(): void {
  nextTick(() => {
    scrollerRef.value?.scrollToItem(props.index, { behavior: 'smooth' });
  });
}

watch(
  () => props.index,
  () => {
    stopVideo();
    isAnalyzing.value = false;
    isAnalyzingModel.value = null;
    if (runTimer) {
      clearInterval(runTimer);
      runTimer = null;
    }
    fetchFaces();
    loadOcr();
    scrollToActiveThumb();
    if (isVideo.value) {
      showInfo.value = false;
      void ensurePort();
    }
  },
);

watch(
  () => props.photos,
  (newPhotos) => {
    if (!Array.isArray(newPhotos) || newPhotos.length === 0) {
      if (visible.value) visible.value = false;
      detectedFaces.value = [];
      return;
    }
    if (props.index >= newPhotos.length) emit('update:index', newPhotos.length - 1);
    if (props.index < 0) emit('update:index', 0);
  },
);

watch(visible, (val) => {
  if (val) {
    fetchFaces();
    scrollToActiveThumb();
  } else {
    detectedFaces.value = [];
  }
});

onMounted(async () => {
  window.addEventListener('keydown', handleKeydown);
  try {
    os.value = await invoke<string>('get_os');
  } catch {
    /* ignore */
  }
  listenForEta();
  try {
    downloadedModels.value = await invoke<string[]>('check_models');
  } catch {
    /* ignore */
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
  if (unlistenEta) unlistenEta();
  clearAnalysisListener();
  if (runTimer) clearInterval(runTimer);
});
</script>

<style scoped>
.ocr-text {
  max-height: 140px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 12px;
  line-height: 1.5;
}

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

.viewer-nav-btn {
  position: absolute;
  z-index: 2000;
}
.top-left {
  top: 20px;
  left: 20px;
}
.top-left-more {
  top: 20px;
  left: 68px;
}
.top-right {
  top: 20px;
  right: 20px;
}

.side-nav-btn {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  z-index: 10;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(4px);
  border-radius: 50%;
}
.side-nav-btn.left {
  left: 20px;
}
.side-nav-btn.right {
  right: 20px;
}

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
  flex: 1;
  width: 100%;
  height: 100%;
  overflow-y: hidden;
  scrollbar-width: none;
}

.thumbnail-rail :deep(.vue-recycle-scroller__item-view) {
  display: flex;
  align-items: center;
}

.thumbnail-rail::-webkit-scrollbar {
  display: none;
}

.info-drawer {
  border-left: 1px solid rgba(0, 0, 0, 0.05);
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
  0% {
    content: '';
  }
  25% {
    content: '.';
  }
  50% {
    content: '..';
  }
  75% {
    content: '...';
  }
  100% {
    content: '';
  }
}
</style>
