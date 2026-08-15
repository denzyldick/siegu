<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border overflow-hidden">
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="on-surface" size="32" class="mr-3">
          <v-icon color="surface" size="small">mdi-robot-outline</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
        $t('settings.performance')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-4">
      <ModelsSection :embedded="embedded" />

      <template v-if="!embedded">
        <v-divider class="my-6 border"></v-divider>

        <div>
          <div class="text-caption font-weight-bold text-disabled mb-4 tracking-widest uppercase">
            {{ $t('settings.indexing_when') }}
          </div>

          <v-row dense class="mb-2" role="radiogroup" aria-label="Indexing mode">
            <v-col v-for="mode in indexingModes" :key="mode.value" cols="6" sm="4" class="pa-1">
              <v-card
                variant="flat"
                class="preset-card rounded-lg"
                :class="{ 'preset-card-active': performance.indexingMode === mode.value }"
                role="radio"
                :tabindex="performance.indexingMode === mode.value ? 0 : -1"
                :aria-checked="performance.indexingMode === mode.value"
                :aria-label="$t('settings.mode_' + mode.value)"
                @click="setIndexingMode(mode.value)"
                @keydown="onModeKeydown(mode.value, $event)"
              >
                <v-card-text class="pa-2 text-center">
                  <div class="text-caption font-weight-bold text-high-emphasis">
                    {{ $t('settings.mode_' + mode.value) }}
                  </div>
                </v-card-text>
              </v-card>
            </v-col>
          </v-row>

          <div class="text-caption text-disabled mb-4">
            {{ $t('settings.mode_' + performance.indexingMode + '_desc') }}
          </div>

          <v-btn
            v-if="performance.indexingMode === 'manual'"
            variant="flat"
            color="primary"
            size="small"
            class="font-weight-bold mb-4"
            prepend-icon="mdi-play"
            :loading="isAnalyzingAll"
            :disabled="isAnyModelProcessing && !isAnalyzingAll"
            @click="runAllModels()"
          >
            {{ $t('settings.analyze_now') }}
          </v-btn>
        </div>

        <v-divider class="my-6 border"></v-divider>

        <div>
          <div class="text-caption font-weight-bold text-disabled mb-4 tracking-widest uppercase">
            {{ $t('settings.speed') }}
          </div>

          <div class="text-body-2 text-medium-emphasis mb-4">
            {{ $t('settings.speed_hint') }}
          </div>

          <v-row dense class="mb-2">
            <v-col v-for="preset in presets" :key="preset.value" cols="4" class="pa-1">
              <v-card
                variant="flat"
                class="preset-card rounded-lg"
                :class="{ 'preset-card-active': currentPreset === preset.value }"
                @click="applyPreset(preset.value)"
              >
                <v-card-text class="pa-2 text-center">
                  <div class="text-body-2 font-weight-bold text-high-emphasis">
                    {{ $t('settings.preset_' + preset.value) }}
                  </div>
                  <div class="text-caption text-disabled preset-desc">
                    {{ $t('settings.preset_' + preset.value + '_desc') }}
                  </div>
                </v-card-text>
              </v-card>
            </v-col>
          </v-row>

          <div class="d-flex align-center justify-space-between mb-1">
            <div class="text-body-2 text-disabled">
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

          <div class="text-caption text-high-emphasis font-weight-bold mt-2">
            {{
              $t('settings.speed_summary', {
                cores: performance.mlThreads,
                photos: performance.scanThreads,
              })
            }}
          </div>
        </div>

        <v-divider class="my-6 border"></v-divider>

        <div>
          <div class="d-flex align-center justify-space-between mb-1">
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
            <div v-if="modelsReloading" class="d-flex align-center">
              <v-progress-circular
                indeterminate
                size="14"
                width="2"
                color="primary"
                class="mr-2"
              ></v-progress-circular>
              <span class="text-caption font-weight-bold text-high-emphasis">{{
                $t('settings.models_reloading')
              }}</span>
            </div>
          </div>

          <v-expand-transition>
            <div v-if="showAdvanced" class="pt-2">
              <div class="d-flex justify-space-between align-center mb-2">
                <div class="text-caption font-weight-bold text-high-emphasis">
                  {{ $t('settings.ml_threads') }}
                </div>
                <v-chip size="small" variant="flat" color="primary" class="font-weight-bold">{{
                  performance.mlThreads
                }}</v-chip>
              </div>
              <v-slider
                :model-value="performance.mlThreads"
                :min="1"
                :max="maxThreads"
                :step="1"
                hide-details
                color="primary"
                @update:model-value="onMlThreadsUpdate"
                @change="onMlThreadsChange"
              ></v-slider>
              <div class="text-caption text-disabled mt-1">
                {{ $t('settings.ml_threads_desc') }}
              </div>

              <div class="d-flex justify-space-between align-center mb-2 mt-4">
                <div class="text-caption font-weight-bold text-high-emphasis">
                  {{ $t('settings.memory_budget') }}
                </div>
                <v-chip size="small" variant="flat" color="primary" class="font-weight-bold">
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
                @update:model-value="onMemoryBudgetUpdate"
                @change="onMemoryBudgetChange"
              ></v-slider>
              <div class="text-caption text-disabled mt-1">
                {{ $t('settings.memory_budget_desc') }}
              </div>

              <div class="d-flex justify-space-between align-center mb-2 mt-4">
                <div class="text-caption font-weight-bold text-high-emphasis">
                  {{ $t('settings.batch_delay') }}
                </div>
                <v-chip size="small" variant="flat" color="primary" class="font-weight-bold">
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
                @update:model-value="onGapUpdate"
                @change="onGapChange"
              ></v-slider>
              <div class="text-caption text-disabled mt-1">
                {{ $t('settings.batch_delay_desc') }}
              </div>

              <div class="d-flex justify-space-between align-center mb-2 mt-4">
                <div class="text-caption font-weight-bold text-high-emphasis">
                  {{ $t('settings.scan_threads') }}
                </div>
                <v-chip size="small" variant="flat" color="primary" class="font-weight-bold">{{
                  performance.scanThreads
                }}</v-chip>
              </div>
              <v-slider
                :model-value="performance.scanThreads"
                :min="1"
                :max="maxThreads"
                :step="1"
                hide-details
                color="primary"
                @update:model-value="onScanThreadsUpdate"
                @change="onScanThreadsChange"
              ></v-slider>
              <div class="text-caption text-disabled mt-1">
                {{ $t('settings.scan_threads_desc') }}
              </div>
            </div>
          </v-expand-transition>
        </div>
      </template>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { storeToRefs } from 'pinia';
import { useI18n } from 'vue-i18n';
import ModelsSection from './ModelsSection.vue';
import { useSettingsStore } from '@/stores/settings';

defineProps<{
  embedded?: boolean;
}>();

const store = useSettingsStore();
const { t } = useI18n();

const { currentPreset, performance, modelsReloading, isAnalyzingAll, isAnyModelProcessing } =
  storeToRefs(store);

const {
  maxThreads,
  applyPreset,
  savePerformanceConfig,
  setIndexingMode,
  setMlThreads,
  setMemoryBudget,
  setScanThreads,
  runAllModels,
  showSnackbar,
} = store;

const presets = [
  { value: 'low' as const },
  { value: 'balanced' as const },
  { value: 'full' as const },
];

const indexingModes = [{ value: 'immediate' }, { value: 'idle' }, { value: 'manual' }];

const showAdvanced = ref(false);

const gapSeconds = computed(() => performance.value.batchDelayMs / 1000);

const memoryBudgetGigabytes = computed(() => performance.value.memoryBudgetMb / 1024);

function onMlThreadsUpdate(value: number | [number, number]): void {
  performance.value.mlThreads = typeof value === 'number' ? value : value[0];
}

function onMlThreadsChange(value: number | [number, number]): void {
  void setMlThreads(typeof value === 'number' ? value : value[0]);
}

function onScanThreadsUpdate(value: number | [number, number]): void {
  performance.value.scanThreads = typeof value === 'number' ? value : value[0];
}

function onScanThreadsChange(value: number | [number, number]): void {
  void setScanThreads(typeof value === 'number' ? value : value[0]);
}

function onGapUpdate(value: number | [number, number]): void {
  const seconds = typeof value === 'number' ? value : value[0];
  performance.value.batchDelayMs = Math.round(seconds * 1000);
}

function onGapChange(value: number | [number, number]): void {
  const seconds = typeof value === 'number' ? value : value[0];
  performance.value.batchDelayMs = Math.round(seconds * 1000);
  showSnackbar(t('settings.gap_applied', { seconds: seconds.toFixed(1) }));
  void savePerformanceConfig();
}

function onMemoryBudgetUpdate(value: number | [number, number]): void {
  const gb = typeof value === 'number' ? value : value[0];
  performance.value.memoryBudgetMb = Math.round(gb * 1024);
}

function onMemoryBudgetChange(value: number | [number, number]): void {
  const gb = typeof value === 'number' ? value : value[0];
  void setMemoryBudget(Math.round(gb * 1024));
}

function onModeKeydown(mode: string, event: KeyboardEvent): void {
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    setIndexingMode(mode);
    return;
  }
  if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
    event.preventDefault();
    moveModeFocus(1);
  } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
    event.preventDefault();
    moveModeFocus(-1);
  }
}

function moveModeFocus(delta: number): void {
  const group = document.querySelector('[role="radiogroup"]');
  if (!group) return;
  const cards = Array.from(group.querySelectorAll<HTMLElement>('[role="radio"]'));
  if (cards.length === 0) return;
  const currentIndex = Math.max(
    0,
    cards.findIndex(
      (card) => document.activeElement === card || card.getAttribute('aria-checked') === 'true',
    ),
  );
  cards[(currentIndex + delta + cards.length) % cards.length].focus();
}
</script>

<style scoped>
.preset-card {
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    background-color 0.18s ease;
}
.preset-card-active {
  border-color: rgb(var(--v-theme-on-surface)) !important;
  box-shadow: inset 0 2px 0 rgb(var(--v-theme-on-surface));
}
.preset-desc {
  line-height: 1.25;
}
</style>
