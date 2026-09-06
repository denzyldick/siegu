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
            color="rgb(var(--v-theme-on-surface))"
            class="viewer-nav-btn top-left"
            @click="close"
          ></v-btn>

          <template v-if="isMobile">
            <v-btn
              icon="mdi-share-variant-outline"
              variant="text"
              color="rgb(var(--v-theme-on-surface))"
              class="viewer-nav-btn top-right"
              @click="handleShare"
            ></v-btn>
          </template>
          <template v-else>
            <v-menu location="bottom start" close-on-content-click>
              <template v-slot:activator="{ props: menuProps }">
                <v-btn
                  v-bind="menuProps"
                  icon="mdi-dots-vertical"
                  variant="text"
                  color="rgba(var(--v-theme-on-surface), 0.6)"
                  class="viewer-nav-btn top-left-more"
                ></v-btn>
              </template>
              <v-list density="compact">
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
              color="rgba(var(--v-theme-on-surface), 0.6)"
              class="viewer-nav-btn top-right"
              @click="showInfo = !showInfo"
            ></v-btn>
          </template>

          <div class="viewer-content-container">
            <v-btn
              v-if="!isMobile"
              icon="mdi-chevron-left"
              variant="text"
              color="rgb(var(--v-theme-on-surface))"
              size="x-large"
              @click="carouselAnimatePrev()"
              class="side-nav-btn left"
            ></v-btn>

            <div
              class="carousel-viewport"
              @touchstart.passive="onCarouselTouchStart"
              @touchmove.passive="onCarouselTouchMove"
              @touchend="onCarouselTouchEnd"
              @click="onCarouselClick"
            >
              <div
                class="carousel-track"
                :style="{ transform: trackTransform, transition: 'none' }"
              >
                <!-- Previous slide -->
                <div class="carousel-slide">
                  <img
                    v-if="prevItem"
                    :key="'prev-' + prevItem.id"
                    :src="prevThumb"
                    class="viewer-thumb-slide"
                    decoding="async"
                    draggable="false"
                  />
                  <img
                    v-if="prevItem && !isItemVideo(prevItem)"
                    :key="'prev-full-' + prevItem.id"
                    :src="prevFull"
                    class="preload-full"
                    loading="eager"
                    decoding="async"
                    draggable="false"
                    aria-hidden="true"
                    @load="onNeighborFullLoad('prev', $event)"
                  />
                </div>

                <!-- Current slide -->
                <div class="carousel-slide">
                  <div v-if="currentPhoto" :key="currentPhoto.id" class="media-frame">
                    <template v-if="!isVideo">
                      <img
                        :src="currentThumb"
                        class="media-thumb"
                        :class="{ 'is-hidden': fullPhotoLoaded }"
                        decoding="async"
                        draggable="false"
                      />
                      <img
                        :src="currentPhotoSrc"
                        class="media-fill"
                        :class="{ 'is-ready': fullPhotoLoaded }"
                        decoding="async"
                        draggable="false"
                        @load="fullPhotoLoaded = true"
                      />
                    </template>
                    <template v-else>
                      <img
                        :src="currentThumb"
                        class="media-thumb"
                        :class="{ 'is-hidden': videoReady }"
                        decoding="async"
                        draggable="false"
                      />
                      <div v-if="computedVideoUrl" class="video-reveal" :class="{ 'is-ready': videoReady }">
                        <VideoPlayer
                          ref="videoPlayerRef"
                          :src="computedVideoUrl"
                          :type="videoType"
                          :auto-play="true"
                          :transcript="photoTranscript"
                          :title="currentPhotoName"
                          @error="onVideoError($event)"
                          @ready="videoReady = true"
                        />
                      </div>
                    </template>
                  </div>
                </div>

                <!-- Next slide -->
                <div class="carousel-slide">
                  <img
                    v-if="nextItem"
                    :key="'next-' + nextItem.id"
                    :src="nextThumb"
                    class="viewer-thumb-slide"
                    decoding="async"
                    draggable="false"
                  />
                  <img
                    v-if="nextItem && !isItemVideo(nextItem)"
                    :key="'next-full-' + nextItem.id"
                    :src="nextFull"
                    class="preload-full"
                    loading="eager"
                    decoding="async"
                    draggable="false"
                    aria-hidden="true"
                    @load="onNeighborFullLoad('next', $event)"
                  />
                </div>
              </div>
            </div>

            <v-btn
              v-if="!isMobile"
              icon="mdi-chevron-right"
              variant="text"
              color="rgb(var(--v-theme-on-surface))"
              size="x-large"
              @click="carouselAnimateNext()"
              class="side-nav-btn right"
            ></v-btn>
          </div>

          <!-- Time period overlay (mobile) -->
          <transition name="fade">
            <div v-if="timePeriodOverlayVisible" class="time-period-overlay">
              {{ timePeriodOverlayLabel }}
            </div>
          </transition>

          <!-- Heart pop animation (mobile) -->
          <transition name="heart-pop">
            <div v-if="heartPopping" class="heart-overlay">
              <v-icon :color="isFavorited ? 'error' : 'rgba(255,255,255,0.7)'" size="80">
                {{ isFavorited ? 'mdi-heart' : 'mdi-heart-outline' }}
              </v-icon>
            </div>
          </transition>

          <!-- Thumbnail rail (desktop only) -->
          <div v-if="!isMobile" class="thumbnail-rail-container">
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

          <!-- Mobile bottom bar -->
          <div v-if="isMobile" class="mobile-bottom-bar">
            <v-btn
              icon
              variant="text"
              :color="isFavorited ? 'error' : 'rgb(var(--v-theme-on-surface))'"
              @click="toggleFavorite"
              size="small"
            >
              <v-icon>{{ isFavorited ? 'mdi-heart' : 'mdi-heart-outline' }}</v-icon>
            </v-btn>
            <v-btn
              icon
              variant="text"
              color="rgb(var(--v-theme-on-surface))"
              @click="showInfo = !showInfo"
              size="small"
            >
              <v-icon>mdi-information-outline</v-icon>
            </v-btn>
            <v-btn
              icon
              variant="text"
              color="rgb(var(--v-theme-on-surface))"
              @click="moreMenuOpen = true"
              size="small"
            >
              <v-icon>mdi-dots-vertical</v-icon>
            </v-btn>
          </div>

          <v-bottom-sheet v-model="moreMenuOpen">
            <v-list density="compact">
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
        </v-main>

        <v-navigation-drawer
          v-if="showInfo"
          v-model="showInfo"
          location="right"
          width="350"
          color="surface"
          class="border-s border info-drawer"
          temporary
        >
          <v-toolbar color="transparent" density="compact">
            <v-toolbar-title class="text-high-emphasis text-subtitle-1 font-weight-bold">{{
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
              <div v-if="isAnalyzing" class="text-caption text-disabled mt-2 text-center">
                <span class="analyzing-dots">{{ $t('media_viewer.analyzing') }}</span>
              </div>
              <div v-else-if="globalEta" class="text-caption text-disabled mt-2 text-center">
                {{ $t('media_viewer.library_indexing', { time: formatEta(globalEta) }) }}
              </div>
            </div>

            <v-divider class="opacity-5 mb-4" v-if="modelChips.length > 0"></v-divider>

            <div class="mb-6" v-if="modelChips.length > 0">
              <div class="text-caption text-disabled mb-3 text-uppercase tracking-widest">
                {{ $t('media_viewer.run_model') }}
              </div>
              <div v-if="isAnalyzingModel" class="d-flex align-center mb-3">
                <v-progress-circular
                  indeterminate
                  size="16"
                  width="2"
                  color="rgba(var(--v-theme-on-surface), 0.7)"
                  class="mr-2"
                ></v-progress-circular>
                <span class="text-body-2 text-high-emphasis font-weight-bold">
                  {{
                    $t('media_viewer.running_model', {
                      model: $t('models.' + isAnalyzingModel + '.title'),
                    })
                  }}
                  <span class="text-caption text-disabled ml-1"
                    >({{ formatElapsed(runStartTime, runTimerTick) }})</span
                  >
                </span>
              </div>
              <div class="d-flex flex-wrap ga-2">
                <v-chip
                  v-for="m in modelChips"
                  :key="m.id"
                  :variant="m.done ? 'tonal' : 'flat'"
                  :color="m.done ? 'success' : 'primary'"
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
              <div class="text-caption text-disabled mb-1 text-uppercase tracking-widest">
                {{ $t('media_viewer.file_details') }}
              </div>
              <div class="d-flex align-start mb-2">
                <v-icon size="small" color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2 mt-1"
                  >mdi-file-document-outline</v-icon
                >
                <div class="text-body-2 text-medium-emphasis word-break-all">
                  {{ currentPhoto?.location }}
                </div>
              </div>
            </div>

            <v-divider class="opacity-5 mb-4"></v-divider>

            <div class="mb-6" v-if="currentPhoto?.caption">
              <div class="text-caption text-disabled mb-1 text-uppercase tracking-widest">
                {{ $t('media_viewer.ai_caption') }}
              </div>
              <div class="text-body-2 text-high-emphasis font-italic">
                "{{ currentPhoto.caption }}"
              </div>
            </div>

            <v-divider class="opacity-5 mb-4" v-if="currentPhoto?.caption"></v-divider>

            <div class="mb-6" v-if="aestheticsDisplay != null">
              <div class="text-caption text-disabled mb-1 text-uppercase tracking-widest">
                {{ $t('media_viewer.aesthetics_score') }}
              </div>
              <div class="d-flex align-center text-body-2 text-medium-emphasis">
                <v-icon size="18" color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2"
                  >mdi-star</v-icon
                >
                <span>{{ aestheticsDisplay }}</span>
              </div>
            </div>

            <v-divider class="opacity-5 mb-4" v-if="aestheticsDisplay != null"></v-divider>

            <div class="mb-6" v-if="photoOcr && !ocrLoading">
              <div class="text-caption text-disabled mb-1 text-uppercase tracking-widest">
                {{ $t('media_viewer.recognized_text') }}
              </div>
              <div class="text-body-2 text-medium-emphasis ocr-text">{{ photoOcr }}</div>
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

            <div class="mb-6" v-if="photoTranscript && !transcriptLoading">
              <div class="text-caption text-disabled mb-1 text-uppercase tracking-widest">
                {{ $t('media_viewer.transcript') }}
              </div>
              <div class="text-body-2 text-medium-emphasis ocr-text">{{ photoTranscript }}</div>
              <v-btn
                size="x-small"
                variant="text"
                class="mt-1 text-none"
                :title="$t('media_viewer.copy_text')"
                @click="copyTranscriptText"
              >
                <v-icon size="14" class="mr-1">mdi-content-copy</v-icon>
                {{ $t('media_viewer.copy') }}
              </v-btn>
            </div>

            <div class="mb-6" v-if="isVideo && !photoTranscript && !transcriptLoading">
              <div class="text-caption text-disabled mb-1 text-uppercase tracking-widest">
                {{ $t('media_viewer.transcript') }}
              </div>
              <div class="text-body-2 text-medium-emphasis">
                {{ $t('media_viewer.transcript_empty') }}
              </div>
            </div>

            <div class="mb-6" v-if="sortedModelTimings.length > 0">
              <div class="text-caption text-disabled mb-2 text-uppercase tracking-widest">
                {{ $t('media_viewer.model_performance') }}
              </div>
              <div v-for="[model, ms] in sortedModelTimings" :key="model" class="d-flex align-center mb-1">
                <span class="text-body-2 text-medium-emphasis" style="width: 90px">
                  {{ model }}
                </span>
                <div class="model-timing-track">
                  <div class="model-timing-fill" :style="{ width: timingBarWidth(ms) }" />
                </div>
                <span class="text-body-2 text-medium-emphasis ml-2" style="width: 52px; text-align: right">
                  {{ ms.toFixed(0) }}ms
                </span>
              </div>
            </div>

            <v-divider
              class="opacity-5 mb-4"
              v-if="(photoTranscript || (isVideo && !transcriptLoading)) && hasExif"
            ></v-divider>

            <div class="mb-6" v-if="hasExif">
              <div class="text-caption text-disabled mb-3 text-uppercase tracking-widest">
                {{ $t('media_viewer.camera_settings') }}
              </div>

              <div class="d-flex align-center mb-4" v-if="exifData.make || exifData.model">
                <v-icon size="small" color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2"
                  >mdi-camera</v-icon
                >
                <span class="text-body-2 text-medium-emphasis"
                  >{{ exifData.make }} {{ exifData.model }}</span
                >
              </div>

              <v-row dense>
                <v-col cols="6" v-if="exifData.date" class="mb-3">
                  <div class="text-caption text-disabled">
                    {{ $t('media_viewer.date_taken') }}
                  </div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.date }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.dimensions" class="mb-3">
                  <div class="text-caption text-disabled">
                    {{ $t('media_viewer.resolution') }}
                  </div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.dimensions }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.iso" class="mb-3">
                  <div class="text-caption text-disabled">{{ $t('media_viewer.iso') }}</div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.iso }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.shutter" class="mb-3">
                  <div class="text-caption text-disabled">{{ $t('media_viewer.shutter') }}</div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.shutter }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.aperture" class="mb-3">
                  <div class="text-caption text-disabled">{{ $t('media_viewer.aperture') }}</div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.aperture }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.focalLength" class="mb-3">
                  <div class="text-caption text-disabled">
                    {{ $t('media_viewer.focal_length') }}
                  </div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.focalLength }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.lens" class="mb-3">
                  <div class="text-caption text-disabled">{{ $t('media_viewer.lens') }}</div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.lens }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.flash" class="mb-3">
                  <div class="text-caption text-disabled">{{ $t('media_viewer.flash') }}</div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.flash }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.whiteBalance" class="mb-3">
                  <div class="text-caption text-disabled">
                    {{ $t('media_viewer.white_balance') }}
                  </div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.whiteBalance }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.meteringMode" class="mb-3">
                  <div class="text-caption text-disabled">
                    {{ $t('media_viewer.metering_mode') }}
                  </div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.meteringMode }}</div>
                </v-col>
                <v-col cols="6" v-if="exifData.software" class="mb-3">
                  <div class="text-caption text-disabled">{{ $t('media_viewer.software') }}</div>
                  <div class="text-body-2 text-medium-emphasis">{{ exifData.software }}</div>
                </v-col>
              </v-row>
            </div>

            <v-divider class="opacity-5 mb-4"></v-divider>

            <div class="mb-6">
              <div class="text-caption text-disabled mb-3 text-uppercase tracking-widest">
                {{ $t('media_viewer.people_in_photo') }}
              </div>
              <div v-if="detectedFaces.length === 0" class="text-body-2 text-disabled font-italic">
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
                  <v-avatar size="56" class="border mb-1">
                    <v-img :src="face.encoded ?? ''" cover></v-img>
                  </v-avatar>
                  <div
                    class="text-caption text-high-emphasis text-truncate text-center w-100 font-weight-bold"
                  >
                    {{ face.person_name || $t('media_viewer.unnamed') }}
                  </div>
                </div>
              </div>
            </div>

            <v-divider class="opacity-5 mb-4"></v-divider>

            <div class="mb-6">
              <div class="text-caption text-disabled mb-3 text-uppercase tracking-widest">
                {{ $t('media_viewer.ai_insights') }}
              </div>

              <div v-if="aiTags.length === 0" class="text-body-2 text-disabled font-italic">
                {{ $t('media_viewer.no_insights') }}
              </div>

              <div v-for="tag in aiTags" :key="tag.name" class="mb-4">
                <div class="d-flex align-center justify-space-between w-100">
                  <span class="text-body-2 text-medium-emphasis text-capitalize">{{
                    tag.name
                  }}</span>
                  <span class="text-caption text-disabled">{{ tag.percent }}%</span>
                </div>
                <v-progress-linear
                  :model-value="tag.percent"
                  color="rgb(var(--v-theme-on-surface))"
                  height="2"
                  rounded
                  class="mt-1 opacity-10"
                ></v-progress-linear>
              </div>
            </div>
          </v-list>
        </v-navigation-drawer>
      </v-layout>
      <v-snackbar v-model="snackbar.show" :timeout="6000" location="bottom" color="primary">
        <div class="d-flex align-center">
          <v-icon
            size="small"
            class="mr-3"
            :color="snackbar.error ? 'error' : 'rgb(var(--v-theme-on-primary))'"
            >{{ snackbar.error ? 'mdi-alert-circle' : 'mdi-check-circle' }}</v-icon
          >
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
import { invoke } from '@/services/invoke';
import { revealItemInDir, openPath } from '@tauri-apps/plugin-opener';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { listen } from '@/services/invoke';
import MediaThumbnail from './MediaThumbnail.vue';
import VideoPlayer from './VideoPlayer.vue';
import { RecycleScroller } from 'vue-virtual-scroller';
import AddToAlbumSheet from '@/components/albums/AddToAlbumSheet.vue';
import { isVideo as checkIsVideo } from '@/composables/useMediaUtils';
import { useMediaUrl } from '@/composables/useMediaUrl';
import { useDoubleTap } from '@/composables/useDoubleTap';
import { useTimePeriods } from '@/composables/useTimePeriods';
import { useSwipeCarousel } from '@/composables/useSwipeCarousel';
import { invalidateMediaUrl } from '@/composables/useMediaUrl';
import { useI18n } from 'vue-i18n';
import type { MediaItem } from '@/types/media';

const { t } = useI18n();
const { mediaSrcRef, ensurePort } = useMediaUrl();

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

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const showInfo = ref(false);
const os = ref('');
const moreMenuOpen = ref(false);
const addToAlbumOpen = ref(false);
const detectedFaces = ref<DetectedFace[]>([]);
const photoOcr = ref('');
const ocrLoading = ref(false);
const photoTranscript = ref('');
const transcriptLoading = ref(false);
const modelTimings = ref<Record<string, number>>({});
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
const videoPlayerRef = ref<InstanceType<typeof VideoPlayer> | null>(null);
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

// ---------------------------------------------------------------------------
// Time periods + touch gestures
// ---------------------------------------------------------------------------

const {
  overlayLabel: timePeriodOverlayLabel,
  overlayVisible: timePeriodOverlayVisible,
  jumpToPrevious,
  jumpToNext,
} = useTimePeriods(() => props.photos);

// ---------------------------------------------------------------------------
// Carousel + touch gestures
// ---------------------------------------------------------------------------

const {
  phase: carouselPhase,
  trackTransform,
  getPrevIndex,
  getNextIndex,
  onTouchStart: carouselOnTouchStart,
  onTouchMove: carouselOnTouchMove,
  onTouchEnd: carouselOnTouchEnd,
  animateNext: carouselAnimateNext,
  animatePrev: carouselAnimatePrev,
  reset: carouselReset,
} = useSwipeCarousel({
  totalItems: () => props.photos.length,
  currentIndex: () => props.index,
  onNavigate: (idx: number) => emit('update:index', idx),
  onVerticalSwipe: (dir: 'up' | 'down') => {
    if (dir === 'up') {
      const target = jumpToPrevious(props.index);
      if (target !== null) emit('update:index', target);
    } else {
      const target = jumpToNext(props.index);
      if (target !== null) emit('update:index', target);
    }
  },
});

const {
  handleTap,
  heartPop: heartPopping,
  cancelPending,
} = useDoubleTap(
  () => {
    /* single tap: no-op in viewer (could toggle chrome) */
  },
  () => {
    toggleFavorite();
  },
);

function onCarouselTouchStart(e: TouchEvent): void {
  carouselOnTouchStart(e);
}

function onCarouselTouchMove(e: TouchEvent): void {
  const result = carouselOnTouchMove(e);
  if (result.defaultPrevented) {
    e.preventDefault();
  }
}

function onCarouselTouchEnd(e: TouchEvent): void {
  carouselOnTouchEnd(e);
}

function onCarouselClick(): void {
  if (carouselPhase.value === 'idle') {
    handleTap();
  }
}

// ---------------------------------------------------------------------------
// Favorites
// ---------------------------------------------------------------------------

const isFavorited = computed((): boolean => !!currentPhoto.value?.favorite);

async function toggleFavorite(): Promise<void> {
  if (!currentPhoto.value) return;
  try {
    const isNow = await invoke<boolean>('toggle_favorite', { id: currentPhoto.value.id });
    emit('update:photo', { ...currentPhoto.value, favorite: isNow } as MediaItem);
  } catch (e) {
    console.error('Failed to toggle favorite', e);
  }
}

// ---------------------------------------------------------------------------
// Share
// ---------------------------------------------------------------------------

async function handleShare(): Promise<void> {
  if (!currentPhoto.value) return;
  const photo = currentPhoto.value;

  // Try native OS share if available
  if (navigator.share) {
    try {
      const ext = photo.location.split('.').pop()?.toLowerCase();
      const mimeMap: Record<string, string> = {
        jpg: 'image/jpeg',
        jpeg: 'image/jpeg',
        png: 'image/png',
        webp: 'image/webp',
        heic: 'image/heic',
        heif: 'image/heif',
        mp4: 'video/mp4',
        mov: 'video/quicktime',
        webm: 'video/webm',
      };
      const mime = mimeMap[ext ?? ''] ?? 'application/octet-stream';

      // Convert file to blob for sharing
      if (!photo.view_only) {
        const { convertFileSrc } = await import('@tauri-apps/api/core');
        const url = convertFileSrc(photo.location);
        const response = await fetch(url);
        const blob = await response.blob();
        const file = new File([blob], photo.location.split('/').pop() ?? 'photo', { type: mime });

        await navigator.share({
          title: photo.caption ?? 'Photo from Siegu',
          files: [file],
        });
        return;
      }
    } catch {
      // User cancelled or share failed — fall through to upsell
    }
  }

  // Fallback: show upsell dialog
  snackbar.value = {
    show: true,
    text: t('media_viewer.share_upsell_hint'),
    error: false,
  };
}

// ---------------------------------------------------------------------------
// Computed
// ---------------------------------------------------------------------------

const isMobile = computed(() => os.value === 'android' || os.value === 'ios');

const visible = computed({
  get: () => props.modelValue,
  set: (val: boolean) => emit('update:modelValue', val),
});

const currentPhoto = computed(() => {
  if (!props.photos || props.photos.length === 0) return null;
  return props.photos[props.index];
});

const currentPhotoRef = computed(() => currentPhoto.value);
const currentThumb = mediaSrcRef(currentPhotoRef, 'thumb');
const currentOriginal = mediaSrcRef(currentPhotoRef, 'original');

const fullPhotoLoaded = ref(false);
const videoReady = ref(false);

watch(
  () => currentPhoto.value?.id ?? '',
  () => {
    fullPhotoLoaded.value = false;
    videoReady.value = false;
  },
);

const currentPhotoName = computed(() => {
  const loc = currentPhoto.value?.location;
  if (!loc) return '';
  return loc.split('/').pop() ?? loc;
});

const isVideo = computed(() => {
  if (!currentPhoto.value?.location) return false;
  return checkIsVideo(currentPhoto.value.location);
});

const isViewOnly = computed((): boolean => !!currentPhoto.value?.view_only);

const aestheticsDisplay = computed((): string | null => {
  const score = currentPhoto.value?.aesthetics_score;
  if (score == null || !Number.isFinite(score)) return null;
  return score.toFixed(1);
});

const sortedModelTimings = computed((): [string, number][] => {
  return Object.entries(modelTimings.value).sort((a, b) => b[1] - a[1]);
});

function timingBarWidth(ms: number): string {
  const max = sortedModelTimings.value[0]?.[1] ?? 1;
  const pct = max > 0 ? (ms / max) * 100 : 0;
  return `${Math.max(3, Math.min(100, pct))}%`;
}

const computedVideoUrl = computed(() => {
  if (!currentPhoto.value || !isVideo.value) return '';
  return currentOriginal.value ?? '';
});

const videoType = computed(() => {
  const ext = currentPhoto.value?.location?.split('.').pop()?.toLowerCase();
  if (ext === 'mp4') return 'video/mp4';
  if (ext === 'webm') return 'video/webm';
  if (ext === 'mov') return 'video/mp4';
  if (ext === 'mkv') return 'video/x-matroska';
  if (ext === 'm4v') return 'video/mp4';
  return undefined;
});

const currentPhotoSrc = computed(() => {
  if (!currentPhoto.value || isVideo.value) return '';
  if (currentPhoto.value.view_only) {
    return currentOriginal.value || currentPhoto.value.encoded || '';
  }
  const ext = currentPhoto.value.location.split('.').pop()?.toLowerCase();
  if (['heic', 'heif'].includes(ext ?? '')) {
    return currentPhoto.value.encoded || currentThumb.value || currentOriginal.value || '';
  }
  return currentOriginal.value || currentPhoto.value.encoded || '';
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

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

function clearAnalysisListener(): void {
  if (unlistenResult) {
    unlistenResult();
    unlistenResult = null;
  }
}

function onVideoError(event: Event): void {
  const target = event.target as HTMLMediaElement | null;
  console.error('[MediaViewer] Failed to load video:', {
    src: target?.currentSrc || target?.src || null,
    code: target?.error?.code ?? null,
    location: currentPhoto.value?.location,
  });
  snackbar.value = { show: true, text: 'Failed to load video', error: true };
}

function stopVideo(): void {
  videoPlayerRef.value?.pause();
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

async function loadTranscript(): Promise<void> {
  if (!currentPhoto.value || !isVideo.value) {
    photoTranscript.value = '';
    return;
  }
  transcriptLoading.value = true;
  try {
    photoTranscript.value = await invoke<string>('get_photo_transcript', { id: currentPhoto.value.id });
  } catch (e) {
    console.error('Failed to fetch transcript', e);
    photoTranscript.value = '';
  } finally {
    transcriptLoading.value = false;
  }
}

async function copyTranscriptText(): Promise<void> {
  if (!photoTranscript.value) return;
  try {
    await navigator.clipboard.writeText(photoTranscript.value);
    snackbar.value = { show: true, text: t('media_viewer.copied'), error: false };
  } catch {
    snackbar.value = { show: true, text: t('media_viewer.copy_failed'), error: true };
  }
}

async function loadModelTimings(): Promise<void> {
  if (!currentPhoto.value) {
    modelTimings.value = {};
    return;
  }
  try {
    modelTimings.value = await invoke<Record<string, number>>('get_model_timings', {
      id: currentPhoto.value.id,
    });
  } catch (e) {
    console.error('Failed to fetch model timings', e);
    modelTimings.value = {};
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
        loadModelTimings();
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
  cancelPending();
  stopVideo();
  carouselReset();
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
    text: t('albums.added_to_album', { collection: albumName }),
  };
}

const moreItems = computed(() => {
  const items: Array<{ key: string; icon: string; action: () => void }> = [];
  if (isViewOnly.value) {
    items.push({
      key: 'restore_original',
      icon: 'mdi-cloud-download-outline',
      action: handleRestoreOriginal,
    });
  }
  items.push(
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
  );
  return items.filter((item) => {
    if (item.key === 'set_wallpaper' && os.value === 'ios') return false;
    if (item.key === 'show_in_explorer' && isMobile.value) return false;
    return true;
  });
});

async function handleRestoreOriginal(): Promise<void> {
  const photo = currentPhoto.value;
  if (!photo) return;
  moreMenuOpen.value = false;
  try {
    await invoke('fetch_original', { id: String(photo.id) });
    snackbar.value = { show: true, text: t('media_viewer.restore_started'), error: false };
  } catch (e) {
    snackbar.value = { show: true, text: String(e), error: true };
  }
}

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

// ---------------------------------------------------------------------------
// Carousel item helpers
// ---------------------------------------------------------------------------

const prevItem = computed((): MediaItem | null => {
  if (props.photos.length === 0) return null;
  return props.photos[getPrevIndex()];
});

const nextItem = computed((): MediaItem | null => {
  if (props.photos.length === 0) return null;
  return props.photos[getNextIndex()];
});

const prevThumb = mediaSrcRef(prevItem, 'thumb');
const nextThumb = mediaSrcRef(nextItem, 'thumb');

function isItemVideo(item: MediaItem): boolean {
  return checkIsVideo(item.location ?? '');
}

// Full-resolution preloads for the neighbour slides. Video neighbours are
// deliberately excluded (resolving their 'original' would preload raw video
// bytes for zero benefit). Passing a computed that yields null lets the shared
// mediaSrcRef skip the fetch entirely instead of pulling the video URL.
const prevPhotoRef = computed(() => (prevItem.value ? (!isItemVideo(prevItem.value) ? prevItem.value : null) : null));
const nextPhotoRef = computed(() => (nextItem.value ? (!isItemVideo(nextItem.value) ? nextItem.value : null) : null));
const prevFull = mediaSrcRef(prevPhotoRef, 'original');
const nextFull = mediaSrcRef(nextPhotoRef, 'original');

// ── Bounded 3-image window ─────────────────────────────────────────────────
// The center keeps its full image mounted; neighbours eager-load theirs in a
// hidden node. When navigation moves one of them out of the window, the keyed
// <img> unmounts (frees the decoded pixels). In guest mode the source is a
// blob: URL, so we also revoke it (and drop the cached URL string) to release
// the raw bytes — otherwise the browser would keep the whole library's blobs.
const windowFullBlobs = new Map<string, string>();

function onNeighborFullLoad(_side: 'prev' | 'next', e: Event): void {
  const target = e.target as HTMLImageElement | null;
  const src = target?.currentSrc || target?.src;
  if (!src || !src.startsWith('blob:')) return;
  const slide = _side === 'prev' ? prevItem.value : nextItem.value;
  if (slide) windowFullBlobs.set(String(slide.id), src);
}

function releaseEvictedNeighbors(): void {
  const ids = new Set<string>();
  const current = currentPhoto.value;
  if (current) ids.add(String(current.id));
  if (prevItem.value) ids.add(String(prevItem.value.id));
  if (nextItem.value) ids.add(String(nextItem.value.id));
  for (const [id, url] of [...windowFullBlobs]) {
    if (ids.has(id)) continue;
    windowFullBlobs.delete(id);
    if (url.startsWith('blob:')) URL.revokeObjectURL(url);
    invalidateMediaUrl(id);
  }
}

watch([() => props.index, () => props.photos], releaseEvictedNeighbors);

function handleKeydown(e: KeyboardEvent): void {
  if (!visible.value) return;
  if (e.key === 'ArrowRight') {
    carouselAnimateNext();
  }
  if (e.key === 'ArrowLeft') {
    carouselAnimatePrev();
  }
  if (e.key === 'ArrowUp') {
    const target = jumpToPrevious(props.index);
    if (target !== null) emit('update:index', target);
  }
  if (e.key === 'ArrowDown') {
    const target = jumpToNext(props.index);
    if (target !== null) emit('update:index', target);
  }
  if (e.key === 'Escape') close();
  if (e.key === 'i') showInfo.value = !showInfo.value;
}

function scrollToActiveThumb(): void {
  nextTick(() => {
    scrollerRef.value?.scrollToItem(props.index, { behavior: 'smooth' });
  });
}

// ---------------------------------------------------------------------------
// Watchers
// ---------------------------------------------------------------------------

watch(
  () => props.index,
  () => {
    cancelPending();
    stopVideo();
    isAnalyzing.value = false;
    isAnalyzingModel.value = null;
    if (runTimer) {
      clearInterval(runTimer);
      runTimer = null;
    }
    fetchFaces();
    loadOcr();
    loadTranscript();
    loadModelTimings();
    scrollToActiveThumb();
    if (isVideo.value) {
      showInfo.value = false;
      void ensurePort();
    }
  },
);

watch(
  () => props.photos,
  (newPhotos, oldPhotos) => {
    if (!Array.isArray(newPhotos) || newPhotos.length === 0) {
      if (visible.value) visible.value = false;
      detectedFaces.value = [];
      return;
    }
    // The library rebuilds its photos array in place when new items arrive
    // mid-browse (photo-received events). Keep showing the photo the user is
    // actually viewing by re-anchoring on its id instead of leaving the old
    // numeric index pointing at a different, newly-inserted item.
    const oldIndex = props.index;
    const oldId = oldPhotos?.[oldIndex]?.id;
    if (oldId != null) {
      const reanchored = newPhotos.findIndex((p) => p.id === oldId);
      if (reanchored !== -1 && reanchored !== oldIndex) {
        emit('update:index', reanchored);
        return;
      }
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
    showInfo.value = false;
    cancelPending();
    carouselReset();
  }
});

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

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

.model-timing-track {
  flex: 1;
  height: 6px;
  border-radius: 3px;
  background: rgba(var(--v-theme-on-surface), 0.12);
  overflow: hidden;
}

.model-timing-fill {
  height: 100%;
  border-radius: 3px;
  background: rgb(var(--v-theme-primary));
  transition: width 0.3s ease;
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

/* Carousel */
.carousel-viewport {
  width: 100%;
  height: 100%;
  overflow: hidden;
  position: relative;
  touch-action: pan-y;
}

.carousel-track {
  display: flex;
  width: 300%;
  height: 100%;
  will-change: transform;
}

.carousel-slide {
  width: 33.3333%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

/* Hidden neighbour preloader: renders the full-res image to warm the HTTP /
   decoded cache without taking layout space. Unmounting the keyed node on
   navigation frees the decoded pixels. */
.preload-full {
  display: none;
}

.viewer-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  user-select: none;
  -webkit-user-drag: none;
  pointer-events: none;
}

.viewer-thumb-slide {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  user-select: none;
  -webkit-user-drag: none;
  pointer-events: none;
}

.media-frame {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
}

.media-thumb {
  position: absolute;
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  user-select: none;
  -webkit-user-drag: none;
  pointer-events: none;
  opacity: 1;
  transition: opacity 0.25s ease;
}

.media-thumb.is-hidden {
  opacity: 0;
}

.media-fill {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  user-select: none;
  -webkit-user-drag: none;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.25s ease;
}

.media-fill.is-ready {
  opacity: 1;
}

.video-reveal {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.35s ease;
  pointer-events: none;
}

.video-reveal.is-ready {
  opacity: 1;
  pointer-events: auto;
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
  background: rgb(var(--v-theme-background));
  backdrop-filter: blur(12px);
  border-top: 1px solid rgba(var(--v-theme-on-surface), 0.12);
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
  border-left: 1px solid rgba(var(--v-theme-on-surface), 0.12);
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

/* Mobile bottom bar */
.mobile-bottom-bar {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 16px;
  padding: 12px 16px 20px;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.4));
  z-index: 100;
}

/* Time period overlay */
.time-period-overlay {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: rgba(0, 0, 0, 0.75);
  backdrop-filter: blur(12px);
  color: white;
  font-size: 1.1rem;
  font-weight: 600;
  padding: 10px 24px;
  border-radius: 24px;
  z-index: 150;
  pointer-events: none;
  white-space: nowrap;
}

/* Heart overlay */
.heart-overlay {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  z-index: 160;
  pointer-events: none;
  filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.5));
}

/* Transitions */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.heart-pop-enter-active {
  transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}
.heart-pop-leave-active {
  transition: opacity 0.4s ease;
}
.heart-pop-enter-from {
  opacity: 0;
  transform: translate(-50%, -50%) scale(0.5);
}
.heart-pop-leave-to {
  opacity: 0;
}
</style>
