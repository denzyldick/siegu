<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-4">
      <div class="text-caption font-weight-bold text-disabled tracking-widest uppercase">
        {{ $t('settings.ai_models') }}
      </div>
      <div v-if="activeModelSummary || pendingCount > 0" class="text-right">
        <div v-if="activeModelSummary" class="text-caption font-weight-bold text-high-emphasis">
          {{ activeModelSummary }}
        </div>
        <div v-else class="text-caption font-weight-bold text-high-emphasis">
          {{ $t('settings.indexing_jobs', { count: formatIndexingCount(pendingCount) }) }}
        </div>
        <div v-if="pendingCount > 0" class="text-caption text-disabled" style="font-size: 10px">
          {{ $t('settings.eta_label', { time: formatEta(globalEta) }) }}
        </div>
      </div>
    </div>

    <v-sheet
      v-if="visibleActivityModel"
      class="ai-activity-strip d-flex align-center justify-space-between px-4 py-3 mb-4 rounded-lg"
      border
    >
      <div class="d-flex align-center min-width-0">
        <v-progress-circular
          v-if="isModelProcessing(visibleActivityModel.id)"
          indeterminate
          size="20"
          width="2"
          color="rgb(var(--v-theme-on-surface))"
          class="mr-3 flex-shrink-0"
        ></v-progress-circular>
        <v-icon v-else size="20" color="rgb(var(--v-theme-on-surface))" class="mr-3 flex-shrink-0">
          {{ getModelActivityIcon(visibleActivityModel.id) }}
        </v-icon>
        <div class="min-width-0">
          <div class="text-caption text-disabled font-weight-bold">
            {{
              isModelProcessing(visibleActivityModel.id)
                ? $t('settings.current_model')
                : $t('settings.latest_model')
            }}
          </div>
          <div class="text-body-2 text-high-emphasis font-weight-bold text-truncate">
            {{ $t('models.' + visibleActivityModel.id + '.title') }} ·
            {{ getModelStatusText(visibleActivityModel.id) }}
          </div>
        </div>
      </div>
      <v-chip size="small" color="primary" variant="flat" class="ml-3 flex-shrink-0">
        {{ getModelStatusLabel(visibleActivityModel.id) }}
      </v-chip>
    </v-sheet>

    <v-sheet class="d-flex align-center justify-space-between px-4 py-3 mb-4 rounded-lg" border>
      <div class="min-width-0 mr-4">
        <div class="text-body-2 font-weight-bold text-high-emphasis">
          {{ $t('settings.analyze_existing') }}
        </div>
        <div class="text-caption text-disabled">
          {{ $t('settings.analyze_existing_hint') }}
        </div>
      </div>
      <v-switch
        :model-value="analyzeExisting"
        @update:model-value="toggleAnalyzeExisting"
        hide-details
        color="primary"
        density="compact"
      ></v-switch>
    </v-sheet>

    <v-row dense>
      <v-col v-for="model in sortedModels" :key="model.id" cols="12" md="6" class="mb-2">
        <v-card
          variant="outlined"
          border
          class="border rounded-lg fill-height d-flex flex-column ai-model-card"
          :class="{
            'ai-model-card-active': isModelActive(model.id),
            'ai-model-card-blocked': isModelBlocked(model.id),
          }"
        >
          <v-card-item class="pb-2">
            <template v-slot:prepend>
              <v-tooltip
                v-if="!downloadedModels.includes(model.id)"
                :text="
                  isModelDownloadBlocked(model.id)
                    ? getModelBlockReason(model.id)
                    : $t('settings.select_for_download')
                "
                location="top"
              >
                <template v-slot:activator="{ props }">
                  <v-checkbox
                    v-bind="props"
                    :model-value="selectedModels.includes(model.id)"
                    @update:model-value="toggleModelSelection(model.id)"
                    :disabled="isModelDownloadBlocked(model.id)"
                    hide-details
                    density="compact"
                    color="primary"
                    class="ma-0 pa-0"
                  ></v-checkbox>
                </template>
              </v-tooltip>
            </template>
            <v-card-title
              class="text-subtitle-1 font-weight-bold d-flex align-center flex-wrap ga-1"
            >
              <span>{{ $t('models.' + model.id + '.title') }}</span>
              <v-chip
                v-if="isModelActive(model.id)"
                size="x-small"
                color="primary"
                variant="flat"
                class="ml-1"
                prepend-icon="mdi-progress-clock"
              >
                {{ getModelStatusLabel(model.id) }}
              </v-chip>
            </v-card-title>
            <template v-slot:append>
              <v-switch
                v-if="downloadedModels.includes(model.id)"
                :model-value="modelEnabled[model.id]"
                @update:model-value="toggleModel(model.id)"
                :disabled="isModelBlocked(model.id)"
                hide-details
                color="primary"
                density="compact"
                :true-value="true"
                :false-value="false"
                :title="
                  isModelBlocked(model.id)
                    ? getModelBlockReason(model.id)
                    : (modelEnabled[model.id]
                        ? $t('settings.disable_model')
                        : $t('settings.enable_model')) +
                      ' ' +
                      $t('models.' + model.id + '.title')
                "
              ></v-switch>
            </template>
          </v-card-item>

          <v-card-text class="py-0 flex-grow-1">
            <div class="text-body-2 text-high-emphasis">
              {{ $t('models.' + model.id + '.desc') }}
            </div>
            <div class="text-caption text-disabled mt-1 font-italic">
              {{ $t('models.' + model.id + '.search') }}
            </div>
            <div class="d-flex align-center justify-space-between mt-2 model-status-line">
              <span class="text-caption text-disabled">{{
                $t('settings.file_size', { size: model.size })
              }}</span>
              <span class="text-caption text-disabled">
                {{ $t('settings.ram_estimate', { size: modelRam[model.id] }) }}
              </span>
              <div class="d-flex align-center">
                <v-icon
                  v-if="isModelBlocked(model.id)"
                  size="18"
                  color="warning"
                  class="mr-1"
                  :title="getModelBlockReason(model.id)"
                  >mdi-alert-circle-outline</v-icon
                >
                <v-icon
                  v-if="
                    downloadedModels.includes(model.id) &&
                    !isModelActive(model.id) &&
                    !isModelDownloading(model.id) &&
                    !isModelBlocked(model.id)
                  "
                  size="18"
                  color="success"
                  :title="$t('settings.ready')"
                  class="mr-1"
                  >mdi-check-circle</v-icon
                >
                <span
                  v-if="getModelStatusText(model.id)"
                  class="text-caption font-weight-bold model-status-text"
                  :class="isModelActive(model.id) ? 'text-high-emphasis' : 'text-disabled'"
                  :title="getModelStatusText(model.id)"
                >
                  {{ getModelStatusText(model.id) }}
                </span>
              </div>
            </div>

            <div v-if="isModelProcessing(model.id)" class="mt-4">
              <div class="d-flex justify-space-between text-caption mb-1">
                <span class="font-weight-bold text-high-emphasis">{{
                  getModelStatusLabel(model.id)
                }}</span>
                <span>{{ getModelProgressText(model.id) }}</span>
              </div>
              <v-progress-linear
                :indeterminate="!hasModelProgressTotal(model.id)"
                :model-value="getModelProgressPercent(model.id)"
                color="rgb(var(--v-theme-on-surface))"
                height="4"
                rounded
              ></v-progress-linear>
            </div>

            <div v-if="isModelDownloading(model.id)" class="mt-4">
              <div
                v-if="getDownloadStats(model.id).bytesText"
                class="d-flex justify-space-between text-caption mb-1"
              >
                <span class="font-weight-bold text-high-emphasis">{{
                  getDownloadStats(model.id).bytesText
                }}</span>
                <span class="text-disabled">{{ getDownloadStats(model.id).rightText }}</span>
              </div>
              <v-progress-linear
                :indeterminate="!hasDownloadProgressTotal(model.id)"
                :model-value="getProgress(model.id)"
                color="rgb(var(--v-theme-on-surface))"
                height="4"
                rounded
              ></v-progress-linear>
            </div>
          </v-card-text>

          <v-card-actions class="pt-2 pb-3 px-4">
            <v-spacer></v-spacer>
            <v-tooltip
              v-if="!downloadedModels.includes(model.id)"
              :text="
                isModelDownloadBlocked(model.id)
                  ? getModelBlockReason(model.id)
                  : $t('settings.download')
              "
              location="top"
            >
              <template v-slot:activator="{ props }">
                <v-btn
                  v-bind="props"
                  variant="flat"
                  size="small"
                  color="primary"
                  icon="mdi-download"
                  :loading="isModelDownloading(model.id)"
                  :disabled="isAnyModelProcessing || isModelDownloadBlocked(model.id)"
                  @click="downloadModels(false, [model.id])"
                />
              </template>
            </v-tooltip>
            <div v-else-if="!embedded" class="d-flex ga-2">
              <v-tooltip :text="$t('settings.redownload')" location="top">
                <template v-slot:activator="{ props }">
                  <v-btn
                    v-bind="props"
                    variant="tonal"
                    size="small"
                    color="secondary"
                    icon="mdi-refresh"
                    :loading="isModelDownloading(model.id)"
                    :disabled="isAnyModelProcessing"
                    @click="downloadModels(true, [model.id])"
                  />
                </template>
              </v-tooltip>
              <v-tooltip
                :text="
                  isModelBlocked(model.id)
                    ? getModelBlockReason(model.id)
                    : isModelProcessing(model.id)
                      ? getModelStatusLabel(model.id)
                      : $t('settings.run_now')
                "
                location="top"
              >
                <template v-slot:activator="{ props }">
                  <v-btn
                    v-bind="props"
                    variant="flat"
                    size="small"
                    color="primary"
                    icon="mdi-play"
                    :loading="isModelProcessing(model.id)"
                    :disabled="
                      (isAnyModelProcessing && !isModelProcessing(model.id)) ||
                      isModelBlocked(model.id)
                    "
                    @click="runModel(model.id)"
                  />
                </template>
              </v-tooltip>
            </div>
          </v-card-actions>
        </v-card>
      </v-col>
    </v-row>

    <div class="d-flex align-center pt-4 mt-2 border-t">
      <v-btn
        v-if="missingSelectedCount > 0"
        variant="flat"
        color="primary"
        size="small"
        class="font-weight-bold"
        prepend-icon="mdi-download-multiple"
        @click="downloadModels(false, selectedModels)"
        :loading="isDownloading"
        :disabled="isAnyModelProcessing"
      >
        {{ $t('settings.download_selected', { count: missingSelectedCount }) }}
      </v-btn>
      <v-btn
        v-else-if="selectedModels.length > 0"
        variant="text"
        color="primary"
        size="small"
        class="font-weight-bold"
        prepend-icon="mdi-check-all"
        disabled
      >
        {{ $t('settings.all_selected_ready') }}
      </v-btn>
      <v-spacer></v-spacer>
      <div class="text-right">
        <div class="d-flex align-center justify-end mb-1">
          <v-progress-circular
            v-if="modelsReloading"
            indeterminate
            size="14"
            width="2"
            color="primary"
            class="mr-2"
          ></v-progress-circular>
          <div
            class="text-caption text-disabled"
            :class="modelsLoaded ? 'font-weight-bold text-high-emphasis' : ''"
          >
            {{
              modelsReloading
                ? $t('settings.models_reloading')
                : modelsLoaded
                  ? $t('settings.memory_total', { size: totalRamEstimate })
                  : $t('settings.no_models_in_memory')
            }}
          </div>
        </div>
        <v-btn
          variant="tonal"
          color="secondary"
          size="small"
          class="font-weight-bold mt-1"
          prepend-icon="mdi-memory"
          :loading="isMemoryFreeing"
          :disabled="!modelsLoaded || isMemoryFreeing || modelsReloading"
          @click="freeMemory()"
        >
          {{ $t('settings.free_memory') }}
        </v-btn>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { storeToRefs } from 'pinia';
import { useSettingsStore } from '@/stores/settings';

defineProps<{
  embedded?: boolean;
}>();

const store = useSettingsStore();

const {
  sortedModels,
  downloadedModels,
  selectedModels,
  modelEnabled,
  analyzeExisting,
  isDownloading,
  isAnyModelProcessing,
  missingSelectedCount,
  pendingCount,
  globalEta,
  visibleActivityModel,
  activeModelSummary,
  modelRam,
  modelsLoaded,
  modelsReloading,
  totalModelRamEstimate: totalRamEstimate,
  isMemoryFreeing,
} = storeToRefs(store);

const {
  isModelProcessing,
  isModelActive,
  isModelDownloading,
  getModelProgressPercent,
  getModelProgressText,
  getModelStatusLabel,
  getModelStatusText,
  getModelActivityIcon,
  getProgress,
  getDownloadStats,
  isModelBlocked,
  getModelBlockReason,
  formatIndexingCount,
  formatEta,
  toggleModel,
  setAnalyzeExisting,
  downloadModels,
  runModel,
  freeMemory,
} = store;

function hasModelProgressTotal(modelId: string): boolean {
  return getModelProgressPercent(modelId) > 0;
}

function hasDownloadProgressTotal(modelId: string): boolean {
  return getDownloadStats(modelId).hasTotal;
}

function isModelDownloadBlocked(modelId: string): boolean {
  return isModelBlocked(modelId) && !downloadedModels.value.includes(modelId);
}

function toggleModelSelection(modelId: string): void {
  const current = [...selectedModels.value];
  const index = current.indexOf(modelId);
  if (index >= 0) {
    current.splice(index, 1);
  } else {
    current.push(modelId);
  }
  store.selectedModels = current;
}

function toggleAnalyzeExisting(value: boolean | null): void {
  void setAnalyzeExisting(value === true);
}
</script>

<style scoped>
.min-width-0 {
  min-width: 0;
}
.ai-model-card {
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    background-color 0.18s ease;
}
.ai-activity-strip {
  border-color: rgba(var(--v-theme-on-surface), 0.12) !important;
}
.ai-model-card-active {
  border-color: rgb(var(--v-theme-on-surface)) !important;
  background-color: rgb(var(--v-theme-background)) !important;
  box-shadow: inset 3px 0 0 rgb(var(--v-theme-on-surface));
}
.ai-model-card-blocked {
  opacity: 0.75;
  border-color: color-mix(in srgb, rgb(var(--v-theme-warning)) 40%, transparent) !important;
}
.model-status-line {
  gap: 12px;
}
.model-status-text {
  flex: 1;
  min-width: 96px;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
