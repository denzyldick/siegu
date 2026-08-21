<script setup lang="ts">
import { computed, ref, nextTick, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useScanStore } from '@/stores/scan';
import { useUiStore } from '@/stores/ui';
import { normalizeIndexingCount, formatEta } from '@/composables/useMediaUtils';
import type { ModelState } from '@/types/scan';

const { t } = useI18n();
const scanStore = useScanStore();
const uiStore = useUiStore();

const logContainer = ref<HTMLElement | null>(null);

watch(
  () => scanStore.logLines.length,
  () => {
    nextTick(() => {
      if (logContainer.value) {
        logContainer.value.scrollTop = logContainer.value.scrollHeight;
      }
    });
  },
);

const stepIndex = computed(() => {
  switch (scanStore.phase) {
    case 'discovering':
      return 0;
    case 'processing':
      return 1;
    case 'indexing':
    case 'paused':
      return 2;
    case 'complete':
      return 3;
    default:
      return 0;
  }
});

const displayEta = computed(() => {
  if (scanStore.indexingEta && scanStore.indexingEta > 0) {
    return formatEta(scanStore.indexingEta);
  }
  return null;
});

const displayRemaining = computed(() => {
  return normalizeIndexingCount(scanStore.indexingCount);
});

const shortDirectoryName = computed(() => {
  if (!scanStore.currentDirectory) return '';
  const parts = scanStore.currentDirectory.split(/[/\\]/);
  return parts[parts.length - 1] || scanStore.currentDirectory;
});

const modelChips = computed(() =>
  Object.entries(scanStore.modelStates).map(([name, state]) => ({
    name,
    ...(state as ModelState),
  })),
);

const recentThumbs = computed(() => scanStore.recentAnalyses);

const currentName = computed(() => {
  const a = scanStore.currentAnalysis;
  if (!a) return '';
  const parts = a.location.split(/[/\\]/);
  return parts[parts.length - 1] || a.id;
});

function thumbSrc(location: string): string {
  return location ? convertFileSrc(location) : '';
}

function chipColor(status: string): string {
  if (status === 'running') return 'primary';
  if (status === 'completed') return 'success';
  return '';
}

function chipIcon(status: string): string {
  if (status === 'running') return 'mdi-cog';
  if (status === 'completed') return 'mdi-check';
  return 'mdi-minus';
}

function handleMinimize(): void {
  scanStore.dismiss();
}

function handleViewLibrary(): void {
  scanStore.resetScanState();
  uiStore.setPage('home');
}

function handlePauseResume(): void {
  if (scanStore.isPaused) {
    scanStore.resume();
  } else {
    scanStore.pause();
  }
}
</script>

<template>
  <v-overlay
    :model-value="scanStore.showFullScreen"
    class="scan-experience-overlay"
    :persistent="true"
    :scrim="true"
    scroll-strategy="block"
  >
    <div class="scan-experience d-flex flex-column align-center justify-center">
      <!-- Minimize button -->
      <v-btn icon variant="text" size="small" class="scan-minimize-btn" @click="handleMinimize">
        <v-icon>mdi-minus</v-icon>
        <v-tooltip activator="parent" location="bottom">
          {{ t('scan.minimize') }}
        </v-tooltip>
      </v-btn>

      <!-- Step indicators -->
      <div class="d-flex align-center ga-3 mb-8">
        <div
          v-for="(label, idx) in [
            t('scan.step_discover'),
            t('scan.step_process'),
            t('scan.step_analyze'),
            t('scan.step_done'),
          ]"
          :key="idx"
          class="step-dot-wrapper d-flex align-center ga-2"
        >
          <div
            class="step-dot"
            :class="{
              'step-active': stepIndex === idx,
              'step-done': stepIndex > idx,
            }"
          >
            <v-icon v-if="stepIndex > idx" size="12" color="white">mdi-check</v-icon>
          </div>
          <span
            class="step-label text-caption"
            :class="stepIndex === idx ? 'text-high-emphasis' : 'text-disabled'"
          >
            {{ label }}
          </span>
          <div v-if="idx < 3" class="step-line" :class="{ 'step-line-done': stepIndex > idx }" />
        </div>
      </div>

      <!-- Main content area -->
      <div class="scan-content text-center" style="max-width: 480px; width: 100%">
        <!-- Discovering phase -->
        <template v-if="scanStore.phase === 'discovering'">
          <div class="scan-icon-container mb-6">
            <v-icon size="72" color="primary" class="scan-pulse">mdi-magnify-scan</v-icon>
          </div>
          <h2 class="text-h4 font-weight-bold text-high-emphasis mb-3">
            {{ t('scan.discovering') }}
          </h2>
          <p class="text-body-1 text-medium-emphasis mb-6">
            {{ t('scan.discovering_desc') }}
          </p>

          <!-- Folder progress -->
          <div v-if="scanStore.folderCount > 0" class="mb-4">
            <div class="text-body-2 text-medium-emphasis mb-2">
              {{
                t('scan.folders_progress', {
                  current: scanStore.currentFolderIndex,
                  total: scanStore.folderCount,
                })
              }}
            </div>
            <v-progress-linear
              :model-value="
                scanStore.folderCount > 0
                  ? (scanStore.currentFolderIndex / scanStore.folderCount) * 100
                  : 0
              "
              color="primary"
              height="6"
              rounded
            />
          </div>

          <!-- Current directory -->
          <div
            v-if="shortDirectoryName"
            class="text-caption text-disabled mb-4 text-truncate"
            style="max-width: 100%"
          >
            {{ shortDirectoryName }}
          </div>

          <!-- Photos found counter -->
          <div v-if="scanStore.filesFound > 0" class="scan-counter mt-4">
            <span class="text-h2 font-weight-black text-primary">{{
              scanStore.filesFound.toLocaleString()
            }}</span>
            <div class="text-body-2 text-medium-emphasis">{{ t('scan.photos_found') }}</div>
          </div>

          <!-- Activity log -->
          <div v-if="scanStore.logLines.length > 0" class="scan-log mt-6" ref="logContainer">
            <div v-for="(line, i) in scanStore.logLines" :key="i" class="scan-log-line">
              {{ line }}
            </div>
          </div>
        </template>

        <!-- Processing phase (models warming up) -->
        <template v-else-if="scanStore.phase === 'processing'">
          <div class="scan-icon-container mb-6">
            <v-progress-circular indeterminate size="72" width="4" color="primary" />
          </div>
          <h2 class="text-h4 font-weight-bold text-high-emphasis mb-3">
            {{ t('scan.processing') }}
          </h2>
          <p class="text-body-1 text-medium-emphasis">
            {{ t('scan.loading_models') }}
          </p>
        </template>

        <!-- Indexing phase -->
        <template v-else-if="scanStore.phase === 'indexing' || scanStore.phase === 'paused'">
          <div class="scan-icon-container mb-6">
            <v-icon
              size="72"
              :color="scanStore.isPaused ? 'warning' : 'primary'"
              class="scan-pulse"
            >
              {{ scanStore.isPaused ? 'mdi-pause-circle' : 'mdi-brain' }}
            </v-icon>
          </div>
          <h2 class="text-h4 font-weight-bold text-high-emphasis mb-3">
            {{ scanStore.isPaused ? t('scan.paused') : t('scan.analyzing') }}
          </h2>
          <p class="text-body-1 text-medium-emphasis mb-6">
            {{ scanStore.isPaused ? t('scan.paused_desc') : t('scan.analyzing_desc') }}
          </p>

          <!-- Live job progress -->
          <div v-if="scanStore.analyzeProgress !== null" class="mb-5 live-block">
            <div class="d-flex justify-space-between text-caption text-medium-emphasis mb-1">
              <span>
                {{
                  t('scan.analyzed_of', {
                    completed: scanStore.jobCompleted.toLocaleString(),
                    total: scanStore.jobTotal.toLocaleString(),
                  })
                }}
              </span>
              <span v-if="scanStore.throughputPerMin !== null">
                {{ t('scan.per_min', { n: scanStore.throughputPerMin.toLocaleString() }) }}
              </span>
            </div>
            <v-progress-linear
              :model-value="scanStore.analyzeProgress"
              color="primary"
              height="6"
              rounded
            />
          </div>

          <!-- Latest analyzed card -->
          <div v-if="scanStore.currentAnalysis && !scanStore.isPaused" class="now-card mb-4">
            <v-img :src="thumbSrc(scanStore.currentAnalysis.location)" cover class="now-thumb">
              <template #placeholder>
                <div class="d-flex align-center justify-center fill-height">
                  <v-progress-circular indeterminate size="16" width="2" />
                </div>
              </template>
            </v-img>
            <div class="now-meta min-width-0">
              <div class="text-caption text-disabled">{{ t('scan.latest_analyzed') }}</div>
              <div class="text-body-2 text-high-emphasis text-truncate">
                {{ currentName }}
              </div>
              <div
                v-if="scanStore.currentAnalysis.models.length"
                class="text-caption text-medium-emphasis text-truncate"
              >
                {{ scanStore.currentAnalysis.models.join(' · ') }}
              </div>
            </div>
          </div>

          <!-- Model pipeline chips -->
          <div v-if="modelChips.length > 0 && !scanStore.isPaused" class="mb-4 live-block">
            <div class="text-caption text-disabled mb-1">{{ t('scan.pipeline') }}</div>
            <div class="d-flex flex-wrap justify-center ga-1">
              <v-chip
                v-for="m in modelChips"
                :key="m.name"
                size="x-small"
                variant="tonal"
                :color="chipColor(m.status)"
              >
                <v-icon start size="12">{{ chipIcon(m.status) }}</v-icon>
                {{ m.name }}
                <span v-if="m.pending > 0" class="ml-1 opacity-60">{{ m.pending }}</span>
                <v-tooltip v-if="m.message" activator="parent" location="top">
                  {{ m.message }}
                </v-tooltip>
              </v-chip>
            </div>
          </div>

          <!-- Recently analyzed strip -->
          <div v-if="recentThumbs.length > 0 && !scanStore.isPaused" class="live-block mb-2">
            <div class="text-caption text-disabled mb-1">{{ t('scan.recently_analyzed') }}</div>
            <div class="d-flex ga-1 justify-center">
              <v-img
                v-for="(r, i) in recentThumbs"
                :key="r.id"
                :src="thumbSrc(r.location)"
                cover
                width="52"
                height="52"
                :class="['recent-thumb', { 'recent-new': i === 0 }]"
              >
                <template #placeholder>
                  <div class="d-flex align-center justify-center fill-height">
                    <v-progress-circular indeterminate size="12" width="2" />
                  </div>
                </template>
              </v-img>
            </div>
          </div>

          <!-- Progress info -->
          <div class="d-flex align-center justify-center ga-6 mb-6">
            <div v-if="displayEta" class="text-body-2 text-medium-emphasis">
              ~{{ displayEta }} {{ t('scan.remaining') }}
            </div>
            <div v-if="displayRemaining > 0" class="text-body-2 text-medium-emphasis">
              {{ displayRemaining.toLocaleString() }} {{ t('scan.photos_left') }}
            </div>
          </div>

          <!-- Action buttons -->
          <div class="d-flex align-center justify-center ga-3">
            <v-btn
              v-if="!scanStore.isPaused"
              variant="outlined"
              color="primary"
              height="44"
              class="px-6"
              @click="handlePauseResume"
            >
              <v-icon start size="18">mdi-pause</v-icon>
              {{ t('scan.pause') }}
            </v-btn>
            <v-btn
              v-else
              variant="flat"
              color="primary"
              height="44"
              class="px-6"
              @click="handlePauseResume"
            >
              <v-icon start size="18">mdi-play</v-icon>
              {{ t('scan.resume') }}
            </v-btn>
            <v-btn variant="tonal" color="error" height="44" class="px-6" @click="scanStore.stop()">
              <v-icon start size="18">mdi-stop</v-icon>
              {{ t('scan.stop') }}
            </v-btn>
          </div>

          <!-- Browse hint -->
          <p class="text-caption text-disabled mt-6">
            {{ t('scan.can_browse') }}
          </p>
        </template>

        <!-- Complete phase -->
        <template v-else-if="scanStore.phase === 'complete'">
          <div class="success-check-animation mb-6">
            <v-icon size="72" color="success">mdi-check-decagram</v-icon>
          </div>
          <h2 class="text-h4 font-weight-bold text-high-emphasis mb-3">
            {{ t('scan.complete_title') }}
          </h2>
          <p class="text-body-1 text-medium-emphasis mb-8">
            {{ t('scan.complete_desc') }}
          </p>
          <v-btn variant="flat" color="primary" height="52" class="px-8" @click="handleViewLibrary">
            <v-icon start>mdi-library</v-icon>
            {{ t('scan.view_library') }}
          </v-btn>
        </template>
      </div>
    </div>
  </v-overlay>
</template>

<style scoped>
.scan-experience-overlay {
  z-index: 2000;
}

.scan-experience-overlay :deep(.v-overlay__scrim) {
  background: rgb(var(--v-theme-background)) !important;
  opacity: 1 !important;
}

.scan-experience-overlay :deep(.v-overlay__content) {
  position: fixed;
  inset: 0;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.scan-experience {
  width: 100%;
  height: 100%;
  position: relative;
  padding: 2rem;
  background: rgb(var(--v-theme-background));
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.scan-minimize-btn {
  position: absolute;
  top: 1.5rem;
  right: 1.5rem;
}

.scan-icon-container {
  display: flex;
  justify-content: center;
}

.scan-pulse {
  animation: pulse-glow 2s ease-in-out infinite;
}

@keyframes pulse-glow {
  0%,
  100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.7;
    transform: scale(1.05);
  }
}

.scan-counter {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.live-block {
  width: 100%;
}

.now-card {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  background: rgba(var(--v-theme-on-surface), 0.04);
  border: 1px solid rgba(var(--v-theme-on-surface), 0.08);
  border-radius: 12px;
  padding: 0.625rem;
  text-align: left;
}

.now-thumb {
  width: 64px;
  height: 64px;
  flex-shrink: 0;
  border-radius: 8px;
  overflow: hidden;
}

.now-meta {
  min-width: 0;
}

.recent-thumb {
  border-radius: 8px;
  overflow: hidden;
  animation: thumb-fade 0.35s ease;
}

.recent-new {
  animation: thumb-pop 0.4s cubic-bezier(0.34, 1.56, 0.64, 1);
}

@keyframes thumb-fade {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes thumb-pop {
  0% {
    transform: scale(0.6);
    opacity: 0;
  }
  100% {
    transform: scale(1);
    opacity: 1;
  }
}

.scan-log {
  width: 100%;
  max-height: 140px;
  overflow-y: auto;
  background: rgba(var(--v-theme-on-surface), 0.04);
  border: 1px solid rgba(var(--v-theme-on-surface), 0.08);
  border-radius: 8px;
  padding: 0.75rem 1rem;
  text-align: left;
}

.scan-log-line {
  font-size: 12px;
  line-height: 1.7;
  color: rgba(var(--v-theme-on-surface), 0.7);
}

.success-check-animation {
  animation: check-bounce 0.6s cubic-bezier(0.68, -0.55, 0.265, 1.55);
}

@keyframes check-bounce {
  0% {
    transform: scale(0);
  }
  50% {
    transform: scale(1.2);
  }
  100% {
    transform: scale(1);
  }
}

.step-dot {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: rgba(var(--v-theme-on-surface), 0.15);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.3s ease;
}

.step-active {
  background: rgb(var(--v-theme-primary));
  box-shadow: 0 0 0 4px rgba(var(--v-theme-primary), 0.2);
}

.step-done {
  background: rgb(var(--v-theme-success));
}

.step-line {
  width: 32px;
  height: 2px;
  background: rgba(var(--v-theme-on-surface), 0.12);
  transition: background 0.3s ease;
}

.step-line-done {
  background: rgb(var(--v-theme-success));
}

.step-label {
  white-space: nowrap;
}
</style>
