<template>
  <v-container :class="embedded ? 'pa-0' : 'pb-16 bg-siegu-main'" fluid>
    <v-row justify="center">
      <v-col cols="12" :md="embedded ? 12 : 8" :lg="embedded ? 12 : 6">
        <div v-if="!embedded" class="d-flex align-center justify-space-between mb-8">
          <div>
            <div class="d-flex align-center mb-1">
              <v-icon color="#18181b" size="28" class="mr-3">mdi-cog-outline</v-icon>
              <h1 class="text-h4 font-weight-bold text-zinc-primary">{{ $t('settings.title') }}</h1>
            </div>
            <div class="text-subtitle-1 text-zinc-secondary">{{ $t('settings.desc') }}</div>
          </div>
        </div>

        <!-- Authorized Folders Card -->
        <v-card v-if="!hideFolderSection" variant="flat" color="#ffffff" rounded="xl" class="mb-6 overflow-hidden border-subtle">
          <v-card-item class="bg-zinc-100 py-4">
            <template v-slot:prepend>
              <div class="siegu-icon-circle-dark mr-3">
                <v-icon color="#ffffff" size="small">mdi-folder-lock</v-icon>
              </div>
            </template>
            <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{ $t('settings.folders') }}</v-card-title>
          </v-card-item>

          <v-card-text class="pt-4">
            <v-expand-transition>
              <div v-if="directories.length > 0">
                <v-list bg-color="transparent">
                  <v-list-item
                    v-for="(directory, index) in directories"
                    :key="directory.value"
                    class="px-0"
                  >
                    <template v-slot:prepend>
                      <v-icon color="#71717a" class="mr-2">mdi-folder</v-icon>
                    </template>
                    <v-list-item-title class="text-zinc-primary font-weight-medium text-truncate">{{ directory.title }}</v-list-item-title>
                    <v-list-item-subtitle class="text-zinc-muted text-caption text-truncate">{{ directory.value }}</v-list-item-subtitle>
                    <template v-slot:append>
                      <v-menu>
                        <template v-slot:activator="{ props }">
                          <v-btn icon="mdi-dots-vertical" variant="text" size="small" color="#71717a" v-bind="props"></v-btn>
                        </template>
                        <v-list size="small" class="siegu-list">
                          <v-list-item @click="remove_directory(directory.value)">
                            <v-list-item-title>{{ $t('settings.remove_folder') }}</v-list-item-title>
                          </v-list-item>
                          <v-list-item @click="remove_directory_full(directory.value)" color="error">
                            <v-list-item-title>{{ $t('settings.wipe_folder') }}</v-list-item-title>
                          </v-list-item>
                        </v-list>
                      </v-menu>
                    </template>
                     <v-divider v-if="index < directories.length - 1" class="border-subtle"></v-divider>
                  </v-list-item>
                </v-list>
              </div>
              <div v-else class="text-center py-8 text-zinc-muted border border-dashed rounded-lg border-subtle">
                <div>{{ $t('settings.no_folders') }}</div>
              </div>
            </v-expand-transition>
          </v-card-text>

          <v-card-actions class="pa-4 bg-zinc-50 border-top-subtle">
            <v-btn
              variant="flat"
              color="#000000"
              theme="dark"
              @click="select_directory"
              block
              height="48"
              class="siegu-btn rounded-xl"
            >
              <div class="d-flex align-center">
                <div class="siegu-icon-circle siegu-icon-circle-sm mr-2">
                  <v-icon size="14" color="white">mdi-folder-plus</v-icon>
                </div>
                <span class="text-white font-weight-bold">{{ $t('settings.add_folder') }}</span>
              </div>
            </v-btn>
          </v-card-actions>
        </v-card>


        <!-- AI Models Card -->
        <v-card v-if="!hideAiSection" variant="flat" color="#ffffff" rounded="xl" class="mb-6 overflow-hidden border-subtle">
          <v-card-item class="bg-zinc-100 py-4">
            <template v-slot:prepend>
              <div class="siegu-icon-circle-dark mr-3">
                <v-icon color="#ffffff" size="small">mdi-robot-outline</v-icon>
              </div>
            </template>
            <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{ $t('settings.ai_models') }}</v-card-title>
            <template v-slot:append v-if="activeModelSummary || pendingCount > 0">
               <div class="text-right">
                  <div v-if="activeModelSummary" class="text-caption font-weight-bold text-black">
                    {{ activeModelSummary }}
                  </div>
                  <div v-else class="text-caption font-weight-bold text-black">{{ $t('settings.indexing_jobs', { count: formatIndexingCount(pendingCount) }) }}</div>
                  <div v-if="pendingCount > 0" class="text-caption text-zinc-muted" style="font-size: 10px;">{{ $t('settings.eta_label', { time: formatEta(globalEta) }) }}</div>
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
                <v-icon
                  v-else
                  size="20"
                  color="#18181b"
                  class="mr-3 flex-shrink-0"
                >
                  {{ getModelActivityIcon(visibleActivityModel.id) }}
                </v-icon>
                <div class="min-width-0">
                  <div class="text-caption text-zinc-muted font-weight-bold">
                    {{ isModelProcessing(visibleActivityModel.id) ? $t('settings.current_model') : $t('settings.latest_model') }}
                  </div>
                  <div class="text-body-2 text-zinc-primary font-weight-bold text-truncate">
                    {{ visibleActivityModel.title }} · {{ getModelStatusText(visibleActivityModel.id) }}
                  </div>
                </div>
              </div>
              <v-chip
                size="small"
                color="black"
                variant="flat"
                class="ml-3 flex-shrink-0"
              >
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
                      <v-checkbox v-model="selectedModels" :value="model.id" hide-details density="compact" color="black" class="ma-0 pa-0"></v-checkbox>
                    </template>
                    <v-card-title class="text-subtitle-1 font-weight-bold d-flex align-center flex-wrap ga-1">
                      <span>{{ $t('models.' + model.id + '.title') }}</span>
                      <v-chip
                        v-if="isModelActive(model.id)"
                        size="x-small"
                        color="black"
                        variant="flat"
                        class="ml-1"
                        prepend-icon="mdi-progress-clock"
                      >
                        {{ getModelStatusLabel(model.id) }}
                      </v-chip>
                      <v-chip
                        v-else-if="downloadedModels.includes(model.id)"
                        size="x-small"
                        color="success"
                        variant="flat"
                        class="ml-1"
                        prepend-icon="mdi-check"
                      >
                        {{ $t('settings.ready') }}
                      </v-chip>
                    </v-card-title>
                    <template v-slot:append>
                      <v-switch
                        v-if="downloadedModels.includes(model.id)"
                        v-model="modelEnabled[model.id]"
                        hide-details
                        color="black"
                        density="compact"
                        @change="toggleModel(model.id)"
                        :true-value="true"
                        :false-value="false"
                        :title="(modelEnabled[model.id] ? $t('settings.disable_model') : $t('settings.enable_model')) + ' ' + $t('models.' + model.id + '.title')"
                      ></v-switch>
                    </template>
                  </v-card-item>

                  <v-card-text class="py-0 flex-grow-1">
                    <div class="text-body-2 text-zinc-primary">{{ $t('models.' + model.id + '.desc') }}</div>
                    <div class="text-caption text-zinc-muted mt-1 font-italic">{{ $t('models.' + model.id + '.search') }}</div>
                    <div class="d-flex align-center justify-space-between mt-2 model-status-line">
                      <span class="text-caption text-zinc-muted">{{ $t('settings.file_size', { size: model.size }) }}</span>
                      <span
                        class="text-caption font-weight-bold model-status-text"
                        :class="isModelActive(model.id) ? 'text-black' : 'text-zinc-muted'"
                        :title="getModelStatusText(model.id)"
                      >
                        {{ getModelStatusText(model.id) }}
                      </span>
                    </div>

                    <!-- Progress Area -->
                    <div v-if="isModelProcessing(model.id)" class="mt-4">
                      <div class="d-flex justify-space-between text-caption mb-1">
                        <span class="font-weight-bold text-black">{{ getModelStatusLabel(model.id) }}</span>
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

                    <!-- Download Progress -->
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
                    <v-btn
                      v-if="!downloadedModels.includes(model.id)"
                      variant="flat"
                      size="small"
                      color="black"
                      prepend-icon="mdi-download"
                      :loading="isModelDownloading(model.id)"
                      :disabled="isAnyModelProcessing"
                      @click="downloadModels(false, [model.id])"
                    >
                      {{ $t('settings.download') }}
                    </v-btn>
                    <div v-else-if="!embedded" class="d-flex ga-2">
                       <v-btn
                        variant="tonal"
                        size="small"
                        color="zinc-muted"
                        icon="mdi-refresh"
                        :loading="isModelDownloading(model.id)"
                        :disabled="isAnyModelProcessing"
                        @click="downloadModels(true, [model.id])"
                        :title="$t('settings.update_model')"
                      ></v-btn>
                      <v-btn
                        variant="tonal"
                        size="small"
                        color="black"
                        prepend-icon="mdi-play"
                        :loading="isModelProcessing(model.id)"
                        :disabled="isAnyModelProcessing && !isModelProcessing(model.id)"
                        @click="runModel(model.id)"
                      >
                        {{ isModelProcessing(model.id) ? getModelStatusLabel(model.id) : $t('settings.run_now') }}
                      </v-btn>
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
              color="black"
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
              color="black"
              size="small"
              class="font-weight-bold"
              prepend-icon="mdi-check-all"
              disabled
            >
              {{ $t('settings.all_selected_ready') }}
            </v-btn>
             <v-spacer></v-spacer>
             <div class="text-caption text-zinc-muted">
                {{ $t('settings.models_ready', { count: downloadedModels.length, total: aiModels.length }) }}
             </div>
          </v-card-actions>
        </v-card>

        <!-- Language Section -->
        <v-card variant="flat" color="#ffffff" rounded="xl" class="mb-6 border-subtle overflow-hidden">
          <v-card-item class="bg-zinc-100 py-4">
            <template v-slot:prepend>
              <div class="siegu-icon-circle-dark mr-3">
                <v-icon color="#ffffff" size="small">mdi-translate</v-icon>
              </div>
            </template>
            <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{ $t('language.label') }}</v-card-title>
          </v-card-item>
          <v-card-text class="pt-4">
            <v-select
              :model-value="currentLang"
              @update:model-value="setLanguage"
              :items="availableLanguages"
              item-title="label"
              item-value="code"
              variant="solo-filled"
              density="compact"
              hide-details
              flat
              rounded="lg"
              class="siegu-field"
            ></v-select>
          </v-card-text>
        </v-card>

        <!-- Maintenance Section -->
        <v-card v-if="!embedded" variant="flat" color="#ffffff" rounded="xl" class="mb-6 border-subtle overflow-hidden">
          <v-card-item class="bg-zinc-100 py-4">
            <template v-slot:prepend>
              <div class="siegu-icon-circle-dark mr-3">
                <v-icon color="#ffffff" size="small">mdi-wrench-outline</v-icon>
              </div>
            </template>
            <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{ $t('settings.maintenance') }}</v-card-title>
          </v-card-item>

          <v-card-text class="pt-2">
            <v-list lines="two" class="bg-transparent">
              <v-list-item class="px-0">
                <template v-slot:title>
                  <span class="font-weight-bold text-zinc-primary">{{ $t('settings.cleanup_db') }}</span>
                </template>
                <template v-slot:subtitle>
                  <span class="text-zinc-secondary">{{ $t('settings.cleanup_db_desc') }}</span>
                </template>
                <template v-slot:append>
                  <v-btn 
                    size="small" 
                    variant="flat" 
                    color="#000000"
                    theme="dark"
                    @click="cleanupDb" 
                    :loading="isCleaning" 
                    class="siegu-btn px-4"
                  >
                    <div class="d-flex align-center">
                      <div class="siegu-icon-circle siegu-icon-circle-md mr-3">
                        <v-icon color="#ffffff" size="small">mdi-wrench-outline</v-icon>
                      </div>
                      <span class="text-white font-weight-bold">{{ $t('settings.clean') }}</span>
                    </div>
                  </v-btn>
                </template>
              </v-list-item>
            </v-list>

            <v-divider class="my-4 border-subtle"></v-divider>

            <!-- Advanced Performance -->
            <div class="mb-6">
              <div class="text-caption font-weight-bold text-zinc-muted mb-4 tracking-widest uppercase">{{ $t('settings.advanced') }}</div>
              <div class="pt-2">
                <div class="d-flex justify-space-between align-center mb-2">
                  <div class="text-caption font-weight-bold text-zinc-primary">{{ $t('settings.scan_threads') }}</div>
                  <v-chip size="x-small" color="#000000" variant="flat" class="font-weight-bold text-white">{{ performance.scanThreads }}</v-chip>
                </div>
                <v-slider
                  v-model="performance.scanThreads"
                  :min="1"
                  :max="maxThreads"
                  :step="1"
                  hide-details
                  color="black"
                  track-color="#f4f4f5"
                  @update:model-value="savePerformanceConfig"
                ></v-slider>

                <v-list-item class="px-0 mt-4">
                  <v-list-item-title class="text-caption font-weight-bold text-zinc-primary">{{ $t('settings.indexing_mode') }}</v-list-item-title>
                  <template v-slot:append>
                    <v-menu offset-y>
                      <template v-slot:activator="{ props }">
                        <v-btn variant="tonal" size="x-small" color="black" v-bind="props" class="font-weight-bold">
                          {{ getModeLabel(performance.indexingMode) }}
                          <v-icon size="12" class="ml-1">mdi-chevron-down</v-icon>
                        </v-btn>
                      </template>
                      <v-list density="compact" class="siegu-list">
                        <v-list-item v-for="mode in indexingModes" :key="mode.value" @click="setIndexingMode(mode.value)">
                          <v-list-item-title class="text-caption" :class="{'font-weight-bold': performance.indexingMode === mode.value}">{{ $t('settings.mode_' + mode.value) }}</v-list-item-title>
                        </v-list-item>
                      </v-list>
                    </v-menu>
                  </template>
                </v-list-item>
              </div>
            </div>

            <v-divider class="my-6 border-subtle" v-if="!embedded"></v-divider>

            <!-- Debug Logs -->
            <div v-if="!embedded">
              <div class="text-caption font-weight-bold text-zinc-muted mb-4 tracking-widest uppercase">{{ $t('settings.system_logs') }}</div>
              <v-sheet
                color="#f4f4f5"
                class="pa-4 rounded-lg overflow-y-auto border-subtle debug-logs-sheet mb-4"
                max-height="300"
              >
                <div v-for="(log, i) in logs" :key="i" :class="log.type === 'error' ? 'text-error' : 'text-zinc-secondary'" class="mb-1" style="font-family: monospace; font-size: 11px;">
                  <span class="text-zinc-muted">[{{ log.time }}]</span> {{ log.message }}
                </div>
                <div v-if="logs.length === 0" class="text-zinc-muted text-center py-4 text-caption">{{ $t('settings.no_logs') }}</div>
              </v-sheet>
              
              <div v-if="logs.length > 0" class="d-flex justify-center">
                <v-btn 
                  variant="text" 
                  size="small" 
                  class="text-none font-weight-bold" 
                  color="error"
                  prepend-icon="mdi-trash-can-outline"
                  @click.stop="clearLogs"
                >
                  {{ $t('settings.clear_logs') }}
                </v-btn>
              </div>
            </div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
    <!-- Download Confirmation Dialog -->
    <v-dialog v-model="downloadDialog.show" max-width="400" rounded="xl">
      <v-card color="#ffffff" border class="border-subtle overflow-hidden">
        <v-card-item class="bg-zinc-100 py-4">
          <template v-slot:prepend>
            <div class="siegu-icon-circle-dark mr-3">
              <v-icon color="#ffffff" size="small">mdi-cloud-download-outline</v-icon>
            </div>
          </template>
          <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{ downloadDialog.title }}</v-card-title>
          <template v-slot:append>
            <v-btn icon="mdi-close" variant="text" size="small" @click="downloadDialog.show = false"></v-btn>
          </template>
        </v-card-item>
        
        <v-card-text class="py-6 text-center">
          <div class="text-subtitle-1 text-zinc-secondary px-2">
            {{ downloadDialog.message }}
          </div>
        </v-card-text>

        <v-card-actions class="pa-4 bg-zinc-50 border-top-subtle ga-2">
          <v-btn variant="tonal" color="zinc-muted" @click="downloadDialog.show = false" class="siegu-btn flex-grow-1" height="44">{{ $t('settings.cancel') }}</v-btn>
          <v-btn variant="flat" color="black" @click="startConfirmedDownload" class="siegu-btn flex-grow-1" height="44">{{ $t('settings.download') }}</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="cleanupDialog.show" max-width="400" rounded="xl">
      <v-card color="#ffffff" border class="border-subtle overflow-hidden">
        <v-card-item class="bg-zinc-100 py-4">
          <template v-slot:prepend>
            <div class="siegu-icon-circle-dark mr-3">
              <v-icon color="#ffffff" size="small">mdi-wrench-outline</v-icon>
            </div>
          </template>
          <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{ $t('settings.clear_db_title') }}</v-card-title>
          <template v-slot:append>
            <v-btn icon="mdi-close" variant="text" size="small" @click="cleanupDialog.show = false"></v-btn>
          </template>
        </v-card-item>
        
        <v-card-text class="py-6 text-center">
          <div class="text-subtitle-1 text-zinc-secondary px-2">
            {{ $t('settings.clear_db_desc') }}
          </div>
        </v-card-text>

        <v-card-actions class="pa-4 bg-zinc-50 border-top-subtle ga-2">
          <v-btn variant="tonal" color="zinc-muted" @click="cleanupDialog.show = false" class="siegu-btn flex-grow-1" height="44">{{ $t('settings.cancel') }}</v-btn>
          <v-btn variant="flat" color="black" @click="startConfirmedCleanup" class="siegu-btn flex-grow-1" height="44">{{ $t('settings.clear') }}</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="removeFolderDialog.show" max-width="400" rounded="xl">
      <v-card color="#ffffff" border class="border-subtle overflow-hidden">
        <v-card-item class="bg-zinc-100 py-4">
          <template v-slot:prepend>
            <div class="siegu-icon-circle-dark mr-3">
              <v-icon color="#ffffff" size="small">mdi-folder-remove-outline</v-icon>
            </div>
          </template>
          <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{ $t('settings.wipe_title') }}</v-card-title>
          <template v-slot:append>
            <v-btn icon="mdi-close" variant="text" size="small" @click="removeFolderDialog.show = false"></v-btn>
          </template>
        </v-card-item>
        
        <v-card-text class="py-6 text-center">
          <div class="text-subtitle-1 text-zinc-secondary px-2">
            <span v-html="$t('settings.wipe_desc')"></span>
          </div>
        </v-card-text>

        <v-card-actions class="pa-4 bg-zinc-50 border-top-subtle ga-2">
          <v-btn variant="tonal" color="zinc-muted" @click="removeFolderDialog.show = false" class="siegu-btn flex-grow-1" height="44">{{ $t('settings.cancel') }}</v-btn>
          <v-btn variant="flat" color="black" @click="startConfirmedRemoveFolder" class="siegu-btn flex-grow-1" height="44">{{ $t('settings.wipe_data') }}</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <FolderPicker
        v-model="showFolderPicker"
        @select="onFolderSelected"
    />
    <v-snackbar v-model="snackbar.show" :timeout="3000" location="bottom" color="black">
      <div class="d-flex align-center">
        <v-icon size="small" class="mr-3" :color="snackbar.error ? 'error' : 'white'">{{ snackbar.error ? 'mdi-alert-circle' : 'mdi-check-circle' }}</v-icon>
        <span class="text-body-2">{{ snackbar.text }}</span>
      </div>
    </v-snackbar>
  </v-container>
</template>

<script>
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { homeDir } from "@tauri-apps/api/path";
import { platform } from "@tauri-apps/plugin-os";
import { listen } from "@tauri-apps/api/event";
import FolderPicker from "./FolderPicker.vue";

export default {
  name: "Setting",
  components: { FolderPicker },
  props: {
    embedded: { type: Boolean, default: false },
    hideAiSection: { type: Boolean, default: false },
    hideFolderSection: { type: Boolean, default: false }
  },
  emits: ["folder-added", "models-ready", "done"],
  data: () => ({
    directories: [],
    showFolderPicker: false,
    isAndroid: false,
    dataDir: "",
    configDir: "",
    checkResults: "",
    isDownloading: false,
    isCleaning: false,
    downloadedModels: [],
    selectedModels: [],
    downloadProgress: {},
    downloadingModels: {},
    pendingCount: 0,
    globalEta: 0,
    snackbar: { show: false, text: '', error: false },
    unlistenLog: null,
    unlistenDownloadProgress: null,
    unlistenDownloadComplete: null,
    unlistenEta: null,
    unlistenProgress: null,
    unlistenModelProgress: null,
    uiClock: null,
    uiNow: Date.now(),
    activeModelId: null,
    activeModelHoldUntil: 0,
    modelProgress: {}, // { model_id: { pending, total } }
    aiModels: [
      { id: 'clip', size: '350MB' },
      { id: 'ultraface', size: '2MB' },
      { id: 'ocr', size: '20MB' },
      { id: 'nsfw', size: '328MB' },
      { id: 'aesthetics', size: '1.6GB' },
      { id: 'blip', size: '329MB' },
      { id: 'yolo', size: '15MB' },
      { id: 'arcface', size: '166MB' },
      { id: 'midas', size: '508MB' },
      { id: 'whisper', size: '31MB' },
    ],
    logs: [],
    modelEnabled: {},
    performance: {
      scanThreads: 4,
      indexingMode: "immediate",
    },
    maxThreads: 8,
    indexingModes: [
      { value: "immediate" },
      { value: "idle" },
      { value: "manual" },
    ],
    downloadDialog: {
      show: false,
      title: "",
      message: "",
      models: []
    },
    cleanupDialog: {
      show: false
    },
    removeFolderDialog: {
      show: false,
      path: ""
    },
    currentLang: localStorage.getItem("siegu_language") || "en",
  }),
  computed: {
    missingSelectedCount() {
      return this.selectedModels.filter(id => !this.downloadedModels.includes(id)).length;
    },
    sortedModels() {
      return [...this.aiModels].sort((a, b) => {
        const aDownloaded = this.downloadedModels.includes(a.id);
        const bDownloaded = this.downloadedModels.includes(b.id);
        const aActive = this.isModelActive(a.id);
        const bActive = this.isModelActive(b.id);
        if (aActive && !bActive) return -1;
        if (!aActive && bActive) return 1;
        if (aDownloaded && !bDownloaded) return -1;
        if (!aDownloaded && bDownloaded) return 1;
        return 0;
      });
    },
    processingModels() {
      return this.aiModels.filter(model => this.isModelProcessing(model.id));
    },
    isAnyModelProcessing() {
      return this.processingModels.length > 0;
    },
    activeModelSummary() {
      if (!this.visibleActivityModel) return "";
      return `${this.getModelStatusLabel(this.visibleActivityModel.id)}: ${this.visibleActivityModel.title}`;
    },
    visibleActivityModel() {
      if (!this.activeModelId) return null;
      const model = this.aiModels.find(m => m.id === this.activeModelId);
      if (!model) return null;
      if (this.isModelProcessing(model.id) || this.uiNow < this.activeModelHoldUntil) return model;
      return null;
    },
    availableLanguages() {
      const codes = ["en", "nl", "fr", "es", "pap"];
      return codes.map(code => ({
        code,
        label: this.$t(`language.${code}`),
      }));
    }
  },
  async mounted() {
    this.uiClock = window.setInterval(() => {
      this.uiNow = Date.now();
    }, 1000);

    this.unlistenLog = await listen("log-message", (event) => {
      const log = {
        time: new Date().toLocaleTimeString(),
        message: event.payload,
        type: event.payload.toLowerCase().includes("error") ? "error" : "info"
      };
      this.logs.unshift(log);
      if (this.logs.length > 100) this.logs.pop();
    });

    this.unlistenDownloadProgress = await listen("download-progress", (event) => {
        const { model, downloaded, total } = event.payload;
        this.downloadProgress = { ...this.downloadProgress, [model]: { downloaded, total } };
    });

    this.unlistenDownloadComplete = await listen("download-complete", () => {
        this.isDownloading = false;
        this.downloadingModels = {};
        this.downloadProgress = {};
        this.checkExistingModels();
        this.$emit('models-ready');
    });

    this.unlistenProgress = await listen("indexing-progress", (event) => {
        this.pendingCount = this.normalizeIndexingCount(event.payload);
    });

    this.unlistenEta = await listen("indexing-eta", (event) => {
        this.globalEta = event.payload;
    });

    this.unlistenModelProgress = await listen("model-progress", (event) => {
        const { model, pending, total, status, message } = event.payload;
        const previous = this.modelProgress[model] || {};
        const normalizedPending = typeof pending === "number" ? pending : previous.pending;
        const normalizedTotal = typeof total === "number" ? total : previous.total;
        const normalizedStatus = status || (normalizedPending > 0 ? "running" : "idle");
        this.activeModelId = model;
        if (["completed", "up_to_date", "unavailable", "error"].includes(normalizedStatus)) {
          this.activeModelHoldUntil = Date.now() + 15000;
        } else {
          this.activeModelHoldUntil = 0;
        }
        this.modelProgress = { 
          ...this.modelProgress, 
          [model]: { 
            ...previous,
            pending: normalizedPending, 
            total: normalizedTotal,
            status: normalizedStatus,
            message: message || previous.message || "",
            updatedAt: Date.now(),
          } 
        };
    });

    this.dataDir = await homeDir();
    this.isAndroid = (await platform()) === 'android';
    await this.checkExistingModels();
    await this.loadPerformanceConfig();
    await this.loadModelEnabledStates();

    this.fetchLogs();
    this.list_directories();
  },
  methods: {
    normalizeIndexingCount(value) {
      const count = Number(value);
      if (!Number.isSafeInteger(count) || count < 0 || count > 1000000) return 0;
      return count;
    },
    formatIndexingCount(value) {
      return this.normalizeIndexingCount(value).toLocaleString();
    },
    setLanguage(code) {
      this.currentLang = code;
      localStorage.setItem("siegu_language", code);
      window.location.reload();
    },
    async fetchLogs() {
      try {
        const logsStr = await invoke("get_logs", { limit: 100 });
        const parsed = JSON.parse(logsStr);
        this.logs = parsed.map(l => ({
          time: new Date(l.timestamp).toLocaleTimeString(),
          message: l.message,
          type: l.level === 'error' ? 'error' : 'info'
        }));
      } catch (err) {}
    },
    async clearLogs() {
      await invoke("clear_logs");
      this.logs = [];
      this.showSnackbar(this.$t('settings.logs_cleared'));
    },
    getModeLabel(val) {
      return this.$t('settings.mode_' + val);
    },
    async loadPerformanceConfig() {
      const configStr = await invoke("get_config");
      const config = JSON.parse(configStr);

      if (config.scan_threads) {
        const val = parseInt(config.scan_threads);
        if (!isNaN(val)) this.performance.scanThreads = val;
      }

      if (config.indexing_mode) {
        this.performance.indexingMode = config.indexing_mode;
      }
    },
    showSnackbar(text, error = false) {
      this.snackbar = { show: true, text, error };
    },
    async savePerformanceConfig() {
      await invoke("save_config", { key: "scan_threads", value: this.performance.scanThreads.toString() });
      this.showSnackbar(this.$t('settings.scan_threads_saved'));
    },
    async setIndexingMode(mode) {
      this.performance.indexingMode = mode;
      await invoke("save_config", { key: "indexing_mode", value: mode });
      if (mode === "manual") {
        await invoke("abort_indexing");
        this.showSnackbar(this.$t('settings.manual_enabled'));
      } else {
        this.showSnackbar(this.$t('settings.mode_set', { mode: this.getModeLabel(mode) }));
      }
    },
    async loadModelEnabledStates() {
      const configStr = await invoke("get_config");
      const config = JSON.parse(configStr);
      const enabled = {};
      for (const m of this.aiModels) {
        const key = "model_enabled_" + m.id;
        enabled[m.id] = config[key] !== "false";
      }
      this.modelEnabled = enabled;
    },
    async toggleModel(modelId) {
      const key = "model_enabled_" + modelId;
      await invoke("save_config", { key, value: this.modelEnabled[modelId] ? "true" : "false" });
      const label = this.$t('models.' + modelId + '.title');
      this.showSnackbar(this.modelEnabled[modelId]
        ? this.$t('settings.model_enabled', { model: label })
        : this.$t('settings.model_disabled', { model: label }));
    },
    async checkExistingModels() {
        const downloaded = await invoke("check_models");
        this.downloadedModels = downloaded;
        this.checkResults = JSON.stringify(downloaded);
        this.selectedModels = ["clip", "ultraface", "ocr", "nsfw", "aesthetics", "yolo", "blip", "arcface", "midas", "whisper"];
    },
    async downloadModels(forceUpdate = false, specificModels = null) {
      let modelsToDownload = specificModels || this.selectedModels;
      if (!forceUpdate && !specificModels) {
        modelsToDownload = ["clip", "ultraface", "ocr", "nsfw", "aesthetics", "yolo", "blip", "arcface", "midas", "whisper"].filter(m => !this.downloadedModels.includes(m));
      }
      if (!modelsToDownload || modelsToDownload.length === 0) return;
      
      this.isDownloading = true;
      this.downloadingModels = {
        ...this.downloadingModels,
        ...Object.fromEntries(modelsToDownload.map(m => [m, true]))
      };
      // Initialize progress tracking for requested models
      modelsToDownload.forEach(m => {
        if (m === 'clip') {
           this.downloadProgress['clip-visual'] = { downloaded: 0, total: 1 };
        } else if (m === 'ocr') {
           this.downloadProgress['ocr-det'] = { downloaded: 0, total: 1 };
        } else {
           this.downloadProgress[m] = { downloaded: 0, total: 1 };
        }
      });

      try {
        await invoke('download_models', { models: modelsToDownload });
      } catch (err) {
        this.isDownloading = false;
        modelsToDownload.forEach(m => {
          delete this.downloadingModels[m];
        });
        this.downloadingModels = { ...this.downloadingModels };
      }
    },
    getProgress(model) {
      if (model === 'clip') {
        const parts = ['clip-visual', 'clip-text', 'clip-tokenizer'];
        let downloaded = 0;
        let total = 0;
        parts.forEach(p => {
          if (this.downloadProgress[p]) {
            downloaded += this.downloadProgress[p].downloaded;
            total += this.downloadProgress[p].total || 0;
          }
        });
        if (total === 0) return this.downloadedModels.includes('clip') ? 100 : 0;
        return (downloaded / total) * 100;
      }
      if (model === 'ocr') {
        const parts = ['ocr-det', 'ocr-rec', 'ocr-dict'];
        let downloaded = 0;
        let total = 0;
        parts.forEach(p => {
          if (this.downloadProgress[p]) {
            downloaded += this.downloadProgress[p].downloaded;
            total += this.downloadProgress[p].total || 0;
          }
        });
        if (total === 0) return this.downloadedModels.includes('ocr') ? 100 : 0;
        return (downloaded / total) * 100;
      }
      const progress = this.downloadProgress[model];
      if (!progress || !progress.total) return this.downloadedModels.includes(model) ? 100 : 0;
      return (progress.downloaded / progress.total) * 100;
    },
    formatBytes(bytes) {
      if (!bytes) return '0 B';
      const k = 1024;
      const sizes = ['B', 'KB', 'MB', 'GB'];
      const i = Math.floor(Math.log(bytes) / Math.log(k));
      return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    },
    formatEta(ms) {
      if (!ms || ms < 0) return 'calculating...';
      const totalSeconds = Math.floor(ms / 1000);
      const hours = Math.floor(totalSeconds / 3600);
      const minutes = Math.floor((totalSeconds % 3600) / 60);
      if (hours > 0) return `${hours}h ${minutes}m`;
      if (minutes > 0) return `${minutes}m`;
      return `${totalSeconds % 60}s`;
    },
    async runModel(modelId) {
      const previous = this.modelProgress[modelId] || {};
      this.activeModelId = modelId;
      this.activeModelHoldUntil = 0;
      this.modelProgress = {
        ...this.modelProgress,
        [modelId]: {
          ...previous,
          pending: previous.pending || null,
          total: previous.total || null,
          status: "starting",
          updatedAt: Date.now(),
        }
      };
      try {
        await invoke("analyze_model", { modelId });
        this.showSnackbar(this.$t('settings.analysis_started', { model: this.$t('models.' + modelId + '.title') }));
      } catch (err) {
        this.modelProgress = {
          ...this.modelProgress,
          [modelId]: {
            ...this.modelProgress[modelId],
            pending: 0,
            status: "idle",
            updatedAt: Date.now(),
          }
        };
        this.showSnackbar(this.$t('settings.analysis_failed', { model: this.$t('models.' + modelId + '.title') }), true);
      }
    },
    isModelProcessing(modelId) {
      const progress = this.modelProgress[modelId];
      return !!progress && (progress.status === "starting" || progress.pending > 0);
    },
    isModelActive(modelId) {
      return this.activeModelId === modelId && (this.isModelProcessing(modelId) || this.uiNow < this.activeModelHoldUntil);
    },
    getModelPending(modelId) {
      return this.modelProgress[modelId]?.pending || 0;
    },
    hasModelProgressTotal(modelId) {
      return (this.modelProgress[modelId]?.total || 0) > 0;
    },
    getModelProgressPercent(modelId) {
      const progress = this.modelProgress[modelId];
      if (!progress || !progress.total) return 0;
      const pending = Math.max(progress.pending || 0, 0);
      return Math.max(0, Math.min(100, ((progress.total - pending) / progress.total) * 100));
    },
    getModelProgressText(modelId) {
      const progress = this.modelProgress[modelId];
      if (!progress || progress.status === "starting") return "Starting";
      if (!progress.total) return `${progress.pending || 0} left`;
      return `${Math.max(progress.total - (progress.pending || 0), 0)} of ${progress.total}`;
    },
    getModelStatusLabel(modelId) {
      const progress = this.modelProgress[modelId];
      if (progress?.status === "starting") return this.$t('settings.model_status_starting');
      if (progress?.status === "completed") return this.$t('settings.model_status_finished');
      if (progress?.status === "up_to_date") return this.$t('settings.model_status_up_to_date');
      if (progress?.status === "unavailable") return this.$t('settings.model_status_unavailable');
      if (progress?.status === "error") return this.$t('settings.model_status_error');
      return this.$t('settings.model_status_running');
    },
    getModelStatusText(modelId) {
      const progress = this.modelProgress[modelId];
      if (this.isModelProcessing(modelId)) return this.getModelProgressText(modelId);
      if (progress?.message) return progress.message;
      if (progress?.status === "completed") return this.$t('settings.model_status_finished');
      if (progress?.status === "up_to_date") return this.$t('settings.model_status_up_to_date');
      if (progress?.status === "unavailable") return this.$t('settings.model_status_not_available');
      if (progress?.status === "error") return this.$t('settings.model_status_error');
      if (progress?.total > 0 && progress?.pending === 0) return this.$t('settings.model_status_finished');
      if (progress?.total === 0 && progress?.pending === 0) return this.$t('settings.model_status_up_to_date');
      if (this.downloadedModels.includes(modelId)) return this.$t('settings.model_status_ready_to_run');
      return this.$t('settings.model_status_not_downloaded');
    },
    getModelProgressText(modelId) {
      const progress = this.modelProgress[modelId];
      if (!progress || progress.status === "starting") return this.$t('settings.model_status_starting');
      if (!progress.total) return this.$t('settings.model_n_remaining', { n: progress.pending || 0 });
      return this.$t('settings.model_n_of_total', { n: Math.max(progress.total - (progress.pending || 0), 0), total: progress.total });
    },
    getModelActivityIcon(modelId) {
      const status = this.modelProgress[modelId]?.status;
      if (status === "completed" || status === "up_to_date") return "mdi-check-circle-outline";
      if (status === "unavailable") return "mdi-alert-circle-outline";
      if (status === "error") return "mdi-alert-outline";
      return "mdi-robot-outline";
    },
    isModelDownloading(modelId) {
      return !!this.downloadingModels[modelId];
    },
    async cleanupDb() {
      this.cleanupDialog.show = true;
    },
    async startConfirmedCleanup() {
      this.cleanupDialog.show = false;
      this.isCleaning = true;
      try {
        await invoke("abort_indexing");
        // Short delay to let threads exit
        await new Promise(r => setTimeout(r, 500));
        await invoke("cleanup_database");
        window.location.reload();
      } catch (err) {
        console.error("Failed to cleanup database:", err);
      } finally {
        this.isCleaning = false;
      }
    },
    async remove_directory_full(path) {
      this.removeFolderDialog.path = path;
      this.removeFolderDialog.show = true;
    },
    async startConfirmedRemoveFolder() {
      const path = this.removeFolderDialog.path;
      this.removeFolderDialog.show = false;
      try {
        await invoke("abort_indexing");
        await new Promise(r => setTimeout(r, 300));
        await invoke("remove_directory_full", { path });
        this.list_directories();
        this.showSnackbar(this.$t('settings.folder_removed'));
      } catch (err) {
        this.showSnackbar(this.$t('settings.folder_removed_failed'), true);
      }
    },
    async select_directory() {
      if (this.isAndroid) {
        this.showFolderPicker = true;
        return;
      }
      try {
        const selection = await open({ multiple: true, directory: true });
        if (Array.isArray(selection)) {
          for (const path of selection) { await invoke("add_directory", { path }); }
        } else if (selection) {
          await invoke("add_directory", { path: selection });
        }
        this.list_directories();
      } catch (err) {}
    },
    list_directories() {
      invoke("list_directories").then((response) => {
        const dirs = JSON.parse(response);
        this.directories = dirs.map(dir => ({ title: dir.split('/').pop() || dir, value: dir }));
        this.$emit('folder-added', this.directories);
      });
    },
    remove_directory(path) {
      invoke("remove_directory", { path }).then(() => {
        this.list_directories();
        this.showSnackbar(this.$t('settings.folder_removed_watch'));
      });
    },
    onFolderSelected(path) {
      invoke("add_directory", { path }).then(() => {
        this.list_directories();
        this.showSnackbar(this.$t('settings.folder_added'));
      });
    }
  },
  beforeUnmount() {
    if (this.uiClock) window.clearInterval(this.uiClock);
    if (this.unlistenLog) this.unlistenLog();
    if (this.unlistenDownloadProgress) this.unlistenDownloadProgress();
    if (this.unlistenDownloadComplete) this.unlistenDownloadComplete();
    if (this.unlistenEta) this.unlistenEta();
    if (this.unlistenProgress) this.unlistenProgress();
    if (this.unlistenModelProgress) this.unlistenModelProgress();
  }
};
</script>

<style scoped>
.bg-zinc-100 {
  background-color: #f4f4f5 !important;
}
.siegu-expansion :deep(.v-expansion-panel-title) {
  border-bottom: 1px solid rgba(0, 0, 0, 0.05);
}
.border-top-subtle {
  border-top: 1px solid rgba(0, 0, 0, 0.05) !important;
}
.ai-model-card {
  transition: border-color 0.18s ease, box-shadow 0.18s ease, background-color 0.18s ease;
}
.ai-activity-strip {
  border-color: rgba(24, 24, 27, 0.18) !important;
}
.ai-model-card-active {
  border-color: #18181b !important;
  background-color: #fafafa !important;
  box-shadow: inset 3px 0 0 #18181b;
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
