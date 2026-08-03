<template>
  <v-card
    variant="flat"
    color="surface"
    rounded="xl"
    class="mb-6 overflow-hidden border-subtle"
  >
    <v-card-item class="bg-zinc-100 py-4">
      <template v-slot:prepend>
        <div class="siegu-icon-circle-dark mr-3">
          <v-icon color="#ffffff" size="small">mdi-robot-outline</v-icon>
        </div>
      </template>
      <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
        $t('settings.ai_models')
      }}</v-card-title>
      <template v-slot:append v-if="activeModelSummary || pendingCount > 0">
        <div class="text-right">
          <div
            v-if="activeModelSummary"
            class="text-caption font-weight-bold text-zinc-primary"
          >
            {{ activeModelSummary }}
          </div>
          <div v-else class="text-caption font-weight-bold text-zinc-primary">
            {{ $t('settings.indexing_jobs', { count: formatIndexingCount(pendingCount) }) }}
          </div>
          <div
            v-if="pendingCount > 0"
            class="text-caption text-zinc-muted"
            style="font-size: 10px"
          >
            {{ $t('settings.eta_label', { time: formatEta(globalEta) }) }}
          </div>
        </div>
      </template>
    </v-card-item>

    <v-card-text class="pt-4">
      <v-sheet
        v-if="visibleActivityModel"
        class="ai-activity-strip d-flex align-center justify-space-between px-4 py-3 mb-4 rounded-lg"
        color="#f4f4f5"
        border
      >
        <div class="d-flex align-center min-width-0">
          <v-progress-circular
            v-if="isModelProcessing(visibleActivityModel.id)"
            indeterminate
            size="20"
            width="2"
            color="black"
            class="mr-3 flex-shrink-0"
          ></v-progress-circular>
          <v-icon v-else size="20" color="#18181b" class="mr-3 flex-shrink-0">
            {{ getModelActivityIcon(visibleActivityModel.id) }}
          </v-icon>
          <div class="min-width-0">
            <div class="text-caption text-zinc-muted font-weight-bold">
              {{
                isModelProcessing(visibleActivityModel.id)
                  ? $t('settings.current_model')
                  : $t('settings.latest_model')
              }}
            </div>
            <div class="text-body-2 text-zinc-primary font-weight-bold text-truncate">
              {{ $t('models.' + visibleActivityModel.id + '.title') }} ·
              {{ getModelStatusText(visibleActivityModel.id) }}
            </div>
          </div>
        </div>
        <v-chip size="small" color="black" variant="flat" class="ml-3 flex-shrink-0">
          {{ getModelStatusLabel(visibleActivityModel.id) }}
        </v-chip>
      </v-sheet>

      <v-row dense>
        <v-col v-for="model in sortedModels" :key="model.id" cols="12" md="6" class="mb-2">
          <v-card
            variant="outlined"
            border
            class="border-subtle rounded-lg fill-height d-flex flex-column ai-model-card"
            :class="{ 'ai-model-card-active': isModelActive(model.id) }"
          >
            <v-card-item class="pb-2">
              <template v-slot:prepend>
                <v-tooltip
                  v-if="!downloadedModels.includes(model.id)"
                  :text="$t('settings.select_for_download')"
                  location="top"
                >
                  <template v-slot:activator="{ props }">
                    <v-checkbox
                      v-bind="props"
                      :model-value="selectedModels.includes(model.id)"
                      @update:model-value="toggleModelSelection(model.id)"
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
                  hide-details
                  color="primary"
                  density="compact"
                  :true-value="true"
                  :false-value="false"
                  :title="
                    (modelEnabled[model.id]
                      ? $t('settings.disable_model')
                      : $t('settings.enable_model')) +
                    ' ' +
                    $t('models.' + model.id + '.title')
                  "
                ></v-switch>
              </template>
            </v-card-item>

            <v-card-text class="py-0 flex-grow-1">
              <div class="text-body-2 text-zinc-primary">
                {{ $t('models.' + model.id + '.desc') }}
              </div>
              <div class="text-caption text-zinc-muted mt-1 font-italic">
                {{ $t('models.' + model.id + '.search') }}
              </div>
              <div class="d-flex align-center justify-space-between mt-2 model-status-line">
                <span class="text-caption text-zinc-muted">{{
                  $t('settings.file_size', { size: model.size })
                }}</span>
                <div class="d-flex align-center">
                  <v-icon
                    v-if="
                      downloadedModels.includes(model.id) &&
                      !isModelActive(model.id) &&
                      !isModelDownloading(model.id)
                    "
                    size="18"
                    color="success"
                    :title="$t('settings.ready')"
                    class="mr-1"
                  >mdi-check-circle</v-icon>
                  <span
                    v-if="getModelStatusText(model.id)"
                    class="text-caption font-weight-bold model-status-text"
                    :class="isModelActive(model.id) ? 'text-zinc-primary' : 'text-zinc-muted'"
                    :title="getModelStatusText(model.id)"
                  >
                    {{ getModelStatusText(model.id) }}
                  </span>
                </div>
              </div>

              <div v-if="isModelProcessing(model.id)" class="mt-4">
                <div class="d-flex justify-space-between text-caption mb-1">
                  <span class="font-weight-bold text-zinc-primary">{{
                    getModelStatusLabel(model.id)
                  }}</span>
                  <span>{{ getModelProgressText(model.id) }}</span>
                </div>
                <v-progress-linear
                  :indeterminate="!hasModelProgressTotal(model.id)"
                  :model-value="getModelProgressPercent(model.id)"
                  color="black"
                  bg-color="#e4e4e7"
                  height="4"
                  rounded
                ></v-progress-linear>
              </div>

              <div v-if="isModelDownloading(model.id)" class="mt-4">
                <v-progress-linear
                  :model-value="getProgress(model.id)"
                  color="black"
                  bg-color="#f4f4f5"
                  height="4"
                  rounded
                ></v-progress-linear>
              </div>
            </v-card-text>

            <v-card-actions class="pt-2 pb-3 px-4">
              <v-spacer></v-spacer>
              <v-tooltip
                v-if="!downloadedModels.includes(model.id)"
                :text="$t('settings.download')"
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
                    :disabled="isAnyModelProcessing"
                    @click="$emit('download-models', false, [model.id])"
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
                      @click="$emit('download-models', true, [model.id])"
                    />
                  </template>
                </v-tooltip>
                <v-tooltip
                  :text="
                    isModelProcessing(model.id)
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
                      :disabled="isAnyModelProcessing && !isModelProcessing(model.id)"
                      @click="$emit('run-model', model.id)"
                    />
                  </template>
                </v-tooltip>
              </div>
            </v-card-actions>
          </v-card>
        </v-col>
      </v-row>
    </v-card-text>

    <v-card-actions class="pa-4 bg-zinc-50 border-top-subtle">
      <v-btn
        v-if="missingSelectedCount > 0"
        variant="flat"
        color="primary"
        size="small"
        class="font-weight-bold"
        prepend-icon="mdi-download-multiple"
        @click="$emit('download-models', false, selectedModels)"
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
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
interface ModelEntry {
  id: string
  size: string
}

const props = defineProps<{
  embedded: boolean
  sortedModels: ModelEntry[]
  downloadedModels: string[]
  selectedModels: string[]
  modelEnabled: Record<string, boolean>
  isDownloading: boolean
  isAnyModelProcessing: boolean
  missingSelectedCount: number
  pendingCount: number
  globalEta: number
  visibleActivityModel: ModelEntry | null
  activeModelSummary: string
  isModelProcessing: (modelId: string) => boolean
  isModelActive: (modelId: string) => boolean
  isModelDownloading: (modelId: string) => boolean
  getModelProgressPercent: (modelId: string) => number
  getModelProgressText: (modelId: string) => string
  getModelStatusLabel: (modelId: string) => string
  getModelStatusText: (modelId: string) => string
  getModelActivityIcon: (modelId: string) => string
  getProgress: (modelId: string) => number
  formatIndexingCount: (value: number) => string
  formatEta: (ms: number) => string
  toggleModel: (modelId: string) => void
}>()

const emit = defineEmits<{
  'download-models': [forceUpdate: boolean, models: string[]]
  'run-model': [modelId: string]
  'update-selected-models': [models: string[]]
}>()

function hasModelProgressTotal(modelId: string): boolean {
  return props.getModelProgressPercent(modelId) > 0
}

function toggleModelSelection(modelId: string): void {
  const current = [...props.selectedModels]
  const index = current.indexOf(modelId)
  if (index >= 0) {
    current.splice(index, 1)
  } else {
    current.push(modelId)
  }
  emit('update-selected-models', current)
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
  border-color: var(--color-border-default) !important;
}
.ai-model-card-active {
  border-color: var(--color-text-primary) !important;
  background-color: var(--color-bg-primary) !important;
  box-shadow: inset 3px 0 0 var(--color-text-primary);
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
