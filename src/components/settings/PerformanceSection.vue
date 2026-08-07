<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border-subtle overflow-hidden">
    <v-card-item class="bg-zinc-100 py-4">
      <template v-slot:prepend>
        <div class="siegu-icon-circle-dark mr-3">
          <v-icon color="var(--color-text-btn)" size="small">mdi-speedometer</v-icon>
        </div>
      </template>
      <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
        $t('settings.performance')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-4">
      <div class="text-body-2 text-zinc-secondary mb-4">
        {{ $t('settings.performance_desc') }}
      </div>

      <v-row dense class="mb-2">
        <v-col v-for="preset in presets" :key="preset.value" cols="4" class="pa-1">
          <v-card
            variant="flat"
            class="preset-card rounded-lg"
            :class="{ 'preset-card-active': currentPreset === preset.value }"
            @click="$emit('apply-preset', preset.value)"
          >
            <v-card-text class="pa-2 text-center">
              <div class="text-body-2 font-weight-bold text-zinc-primary">
                {{ $t('settings.preset_' + preset.value) }}
              </div>
              <div class="text-caption text-zinc-muted preset-desc">
                {{ $t('settings.preset_' + preset.value + '_desc') }}
              </div>
            </v-card-text>
          </v-card>
        </v-col>
      </v-row>

      <div class="d-flex align-center justify-space-between mb-1">
        <div class="text-body-2 text-zinc-muted">
          {{
            currentPreset === 'custom'
              ? $t('settings.preset_custom_desc')
              : $t('settings.preset_' + currentPreset + '_desc')
          }}
        </div>
        <v-chip
          v-if="currentPreset === 'custom'"
          size="small"
          variant="tonal"
          color="primary"
          class="font-weight-bold"
        >
          {{ $t('settings.preset_custom') }}
        </v-chip>
      </div>

      <v-divider class="my-4 border-subtle"></v-divider>

      <ModelsSection
        :embedded="embedded"
        :sorted-models="sortedModels"
        :downloaded-models="downloadedModels"
        :selected-models="selectedModels"
        :model-enabled="modelEnabled"
        :is-downloading="isDownloading"
        :is-any-model-processing="isAnyModelProcessing"
        :missing-selected-count="missingSelectedCount"
        :pending-count="pendingCount"
        :global-eta="globalEta"
        :visible-activity-model="visibleActivityModel"
        :active-model-summary="activeModelSummary"
        :is-model-processing="isModelProcessing"
        :is-model-active="isModelActive"
        :is-model-downloading="isModelDownloading"
        :get-model-progress-percent="getModelProgressPercent"
        :get-model-progress-text="getModelProgressText"
        :get-model-status-label="getModelStatusLabel"
        :get-model-status-text="getModelStatusText"
        :get-model-activity-icon="getModelActivityIcon"
        :get-progress="getProgress"
        :get-download-stats="getDownloadStats"
        :is-model-blocked="isModelBlocked"
        :get-model-block-reason="getModelBlockReason"
        :format-indexing-count="formatIndexingCount"
        :format-eta="formatEta"
        :toggle-model="toggleModel"
        :model-ram="modelRam"
        :models-loaded="modelsLoaded"
        :total-ram-estimate="totalRamEstimate"
        :is-memory-freeing="isMemoryFreeing"
        @download-models="(force, models) => emit('download-models', force, models)"
        @run-model="(id) => emit('run-model', id)"
        @update-selected-models="(models) => emit('update-selected-models', models)"
        @free-memory="emit('free-memory')"
      />

      <v-divider class="my-4 border-subtle"></v-divider>

      <div>
        <v-btn
          variant="text"
          size="small"
          class="text-none font-weight-bold pa-0"
          color="primary"
          @click="showAdvanced = !showAdvanced"
        >
          <v-icon size="16" class="mr-1">
            {{ showAdvanced ? 'mdi-chevron-up' : 'mdi-chevron-down' }}
          </v-icon>
          {{ $t('settings.' + (showAdvanced ? 'hide_advanced' : 'show_advanced')) }}
        </v-btn>

        <v-expand-transition>
          <div v-if="showAdvanced" class="pt-2">
            <div class="d-flex justify-space-between align-center mb-2">
              <div class="text-caption font-weight-bold text-zinc-primary">
                {{ $t('settings.batch_delay') }}
              </div>
              <v-chip size="small" variant="flat" class="bg-btn font-weight-bold">
                {{ gapSeconds.toFixed(1) }}s
              </v-chip>
            </div>
            <v-slider
              :model-value="gapSeconds"
              :min="0"
              :max="2"
              :step="0.1"
              hide-details
              color="primary"
              track-color="var(--color-bg-zinc-100)"
              @update:model-value="onGapChange"
            ></v-slider>
            <div class="text-caption text-zinc-muted mt-1">
              {{ $t('settings.batch_delay_desc') }}
            </div>

            <div class="d-flex justify-space-between align-center mb-2 mt-4">
              <div class="text-caption font-weight-bold text-zinc-primary">
                {{ $t('settings.memory_budget') }}
              </div>
              <v-chip size="small" variant="flat" class="bg-btn font-weight-bold">
                {{
                  performance.memoryBudgetMb === 0
                    ? $t('settings.memory_budget_none')
                    : performance.memoryBudgetMb + ' MB'
                }}
              </v-chip>
            </div>
            <v-slider
              :model-value="memoryBudgetGigabytes"
              :min="0"
              :max="4"
              :step="0.25"
              hide-details
              color="primary"
              track-color="var(--color-bg-zinc-100)"
              @update:model-value="onMemoryBudgetChange"
            ></v-slider>
            <div class="text-caption text-zinc-muted mt-1">
              {{ $t('settings.memory_budget_desc') }}
            </div>

            <div class="d-flex justify-space-between align-center mb-2 mt-4">
              <div class="text-caption font-weight-bold text-zinc-primary">
                {{ $t('settings.ml_threads') }}
              </div>
              <v-chip size="small" variant="flat" class="bg-btn font-weight-bold">
                {{ performance.mlThreads }}
              </v-chip>
            </div>
            <v-slider
              :model-value="performance.mlThreads"
              :min="1"
              :max="maxThreads"
              :step="1"
              hide-details
              color="primary"
              track-color="var(--color-bg-zinc-100)"
              @update:model-value="onMlThreadsChange"
            ></v-slider>
          </div>
        </v-expand-transition>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import ModelsSection from './ModelsSection.vue';
import type { DownloadStats, PerformanceConfig, PerformancePreset } from '@/types/settings';

interface ModelEntry {
  id: string;
  size: string;
}

const props = defineProps<{
  embedded: boolean;
  performance: PerformanceConfig;
  currentPreset: PerformancePreset;
  maxThreads: number;
  sortedModels: ModelEntry[];
  downloadedModels: string[];
  selectedModels: string[];
  modelEnabled: Record<string, boolean>;
  isDownloading: boolean;
  isAnyModelProcessing: boolean;
  missingSelectedCount: number;
  pendingCount: number;
  globalEta: number;
  visibleActivityModel: ModelEntry | null;
  activeModelSummary: string;
  isModelProcessing: (modelId: string) => boolean;
  isModelActive: (modelId: string) => boolean;
  isModelDownloading: (modelId: string) => boolean;
  getModelProgressPercent: (modelId: string) => number;
  getModelProgressText: (modelId: string) => string;
  getModelStatusLabel: (modelId: string) => string;
  getModelStatusText: (modelId: string) => string;
  getModelActivityIcon: (modelId: string) => string;
  getDownloadStats: (modelId: string) => DownloadStats;
  getProgress: (modelId: string) => number;
  isModelBlocked: (modelId: string) => boolean;
  getModelBlockReason: (modelId: string) => string;
  formatIndexingCount: (value: number) => string;
  formatEta: (ms: number) => string;
  toggleModel: (modelId: string) => void;
  modelRam: Record<string, string>;
  modelsLoaded: boolean;
  totalRamEstimate: string;
  isMemoryFreeing: boolean;
}>();

const emit = defineEmits<{
  'apply-preset': [preset: string];
  'update-batch-delay': [valueMs: number];
  'update-memory-budget': [valueMb: number];
  'update-ml-threads': [value: number];
  'download-models': [forceUpdate: boolean, models: string[]];
  'run-model': [modelId: string];
  'update-selected-models': [models: string[]];
  'free-memory': [];
}>();

const presets = [{ value: 'low' }, { value: 'balanced' }, { value: 'full' }];

const showAdvanced = ref(false);

const gapSeconds = computed(() => props.performance.batchDelayMs / 1000);

const memoryBudgetGigabytes = computed(() => props.performance.memoryBudgetMb / 1024);

function onGapChange(value: number | [number, number]): void {
  const seconds = typeof value === 'number' ? value : value[0];
  emit('update-batch-delay', Math.round(seconds * 1000));
}

function onMemoryBudgetChange(value: number | [number, number]): void {
  const gb = typeof value === 'number' ? value : value[0];
  emit('update-memory-budget', Math.round(gb * 1024));
}

function onMlThreadsChange(value: number | [number, number]): void {
  emit('update-ml-threads', typeof value === 'number' ? value : value[0]);
}
</script>

<style scoped>
.preset-card {
  border: 1px solid var(--color-border-subtle);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    background-color 0.18s ease;
}
.preset-card-active {
  border-color: var(--color-text-primary) !important;
  box-shadow: inset 0 2px 0 var(--color-text-primary);
}
.preset-desc {
  line-height: 1.25;
}
</style>
