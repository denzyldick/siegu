<template>
  <v-container :class="embedded ? 'pa-0' : 'pb-16 bg-siegu-main'" fluid>
    <v-row justify="center">
      <v-col cols="12" :md="embedded ? 12 : 8" :lg="embedded ? 12 : 6">
        <div v-if="!embedded" class="d-flex align-center justify-space-between mb-8">
          <div>
            <div class="d-flex align-center mb-1">
              <v-icon color="#18181b" size="28" class="mr-3">mdi-cog-outline</v-icon>
              <h1 class="text-h4 font-weight-bold text-zinc-primary">Settings</h1>
            </div>
            <div class="text-subtitle-1 text-zinc-secondary">Configure your library and AI preferences</div>
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
            <v-card-title class="text-h6 text-zinc-primary font-weight-bold">Authorized Folders</v-card-title>
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
                            <v-list-item-title>Remove Folder Reference</v-list-item-title>
                          </v-list-item>
                          <v-list-item @click="remove_directory_full(directory.value)" color="error">
                            <v-list-item-title>Wipe Local Data & Remove</v-list-item-title>
                          </v-list-item>
                        </v-list>
                      </v-menu>
                    </template>
                     <v-divider v-if="index < directories.length - 1" class="border-subtle"></v-divider>
                  </v-list-item>
                </v-list>
              </div>
              <div v-else class="text-center py-8 text-zinc-muted border border-dashed rounded-lg border-subtle">
                <div>No folders added yet.</div>
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
                <span class="text-white font-weight-bold">Add Folder</span>
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
            <v-card-title class="text-h6 text-zinc-primary font-weight-bold">AI Models</v-card-title>
            <template v-slot:append v-if="pendingCount > 0">
               <div class="text-right">
                  <div class="text-caption font-weight-bold text-black">Indexing {{ pendingCount }} photos</div>
                  <div class="text-caption text-zinc-muted" style="font-size: 10px;">ETA: {{ formatEta(globalEta) }}</div>
               </div>
            </template>
          </v-card-item>

          <v-card-text class="pt-4">
            <v-row dense>
              <v-col v-for="model in aiModels" :key="model.id" cols="12" md="6" class="mb-2">
                <v-card variant="outlined" border class="border-subtle rounded-lg fill-height d-flex flex-column">
                  <v-card-item class="pb-2">
                    <template v-slot:prepend>
                      <v-checkbox v-model="selectedModels" :value="model.id" hide-details density="compact" color="black" class="ma-0 pa-0"></v-checkbox>
                    </template>
                    <v-card-title class="text-subtitle-1 font-weight-bold d-flex align-center">
                      {{ model.title }}
                      <v-chip v-if="downloadedModels.includes(model.id)" size="x-small" color="success" variant="flat" class="ml-2">Ready</v-chip>
                    </v-card-title>
                  </v-card-item>

                  <v-card-text class="py-0 flex-grow-1">
                    <div class="text-body-2 text-zinc-primary">{{ model.desc }}</div>
                    <div class="text-caption text-zinc-muted mt-1 font-italic">{{ model.search }}</div>
                    <div class="text-caption text-zinc-muted mt-1">File size: {{ model.size }}</div>

                    <!-- Progress Area -->
                    <div v-if="isModelProcessing(model.id)" class="mt-4">
                      <div class="d-flex justify-space-between text-caption mb-1">
                        <span class="font-weight-bold text-black">Running...</span>
                        <span>{{ getModelPending(model.id) }} left</span>
                      </div>
                      <v-progress-linear
                        indeterminate
                        color="black"
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
                      @click="downloadModels(false, [model.id])"
                    >
                      Download
                    </v-btn>
                    <div v-else class="d-flex ga-2">
                       <v-btn
                        variant="tonal"
                        size="small"
                        color="zinc-muted"
                        icon="mdi-refresh"
                        :loading="isModelDownloading(model.id)"
                        @click="downloadModels(true, [model.id])"
                        title="Update Model"
                      ></v-btn>
                      <v-btn
                        variant="tonal"
                        size="small"
                        color="black"
                        prepend-icon="mdi-play"
                        :disabled="isModelProcessing(model.id)"
                        @click="runModel(model.id)"
                      >
                        Run Now
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
            >
              Download Selected ({{ missingSelectedCount }})
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
              All Selected Ready
            </v-btn>
             <v-spacer></v-spacer>
             <div class="text-caption text-zinc-muted">
                {{ downloadedModels.length }} of {{ aiModels.length }} models ready
             </div>
          </v-card-actions>
        </v-card>

        <!-- Maintenance Section -->
        <v-card v-if="!embedded" variant="flat" color="#ffffff" rounded="xl" class="mb-6 border-subtle overflow-hidden">
          <v-card-item class="bg-zinc-100 py-4">
            <template v-slot:prepend>
              <div class="siegu-icon-circle-dark mr-3">
                <v-icon color="#ffffff" size="small">mdi-wrench-outline</v-icon>
              </div>
            </template>
            <v-card-title class="text-h6 text-zinc-primary font-weight-bold">Maintenance</v-card-title>
          </v-card-item>

          <v-card-text class="pt-2">
            <v-list lines="two" class="bg-transparent">
              <v-list-item class="px-0">
                <template v-slot:title>
                  <span class="font-weight-bold text-zinc-primary">Background Sync</span>
                </template>
                <template v-slot:subtitle>
                  <span class="text-zinc-secondary">Allow syncing when app is in background</span>
                </template>
                <template v-slot:append>
                  <v-switch v-model="bgSync" hide-details color="black" inset density="compact"></v-switch>
                </template>
              </v-list-item>

              <v-list-item class="px-0">
                <template v-slot:title>
                  <span class="font-weight-bold text-zinc-primary">Cleanup Database</span>
                </template>
                <template v-slot:subtitle>
                  <span class="text-zinc-secondary">Optimize storage and remove orphaned entries</span>
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
                      <span class="text-white font-weight-bold">Clean</span>
                    </div>
                  </v-btn>
                </template>
              </v-list-item>
            </v-list>

            <v-divider class="my-4 border-subtle"></v-divider>

            <!-- Advanced Performance -->
            <div class="mb-6">
              <div class="text-caption font-weight-bold text-zinc-muted mb-4 tracking-widest uppercase">Advanced Performance</div>
              <div class="pt-2">
                <div class="d-flex justify-space-between align-center mb-2">
                  <div class="text-caption font-weight-bold text-zinc-primary">Scanning Threads</div>
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
                  <v-list-item-title class="text-caption font-weight-bold text-zinc-primary">Indexing Mode</v-list-item-title>
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
                          <v-list-item-title class="text-caption" :class="{'font-weight-bold': performance.indexingMode === mode.value}">{{ mode.title }}</v-list-item-title>
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
              <div class="text-caption font-weight-bold text-zinc-muted mb-4 tracking-widest uppercase">System Logs</div>
              <v-sheet
                color="#f4f4f5"
                class="pa-4 rounded-lg overflow-y-auto border-subtle debug-logs-sheet mb-4"
                max-height="300"
              >
                <div v-for="(log, i) in logs" :key="i" :class="log.type === 'error' ? 'text-error' : 'text-zinc-secondary'" class="mb-1" style="font-family: monospace; font-size: 11px;">
                  <span class="text-zinc-muted">[{{ log.time }}]</span> {{ log.message }}
                </div>
                <div v-if="logs.length === 0" class="text-zinc-muted text-center py-4 text-caption">No logs recorded yet.</div>
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
                  Clear Log History
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
          <v-btn variant="tonal" color="zinc-muted" @click="downloadDialog.show = false" class="siegu-btn flex-grow-1" height="44">Cancel</v-btn>
          <v-btn variant="flat" color="black" @click="startConfirmedDownload" class="siegu-btn flex-grow-1" height="44">Download</v-btn>
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
          <v-card-title class="text-h6 text-zinc-primary font-weight-bold">Clear Database?</v-card-title>
          <template v-slot:append>
            <v-btn icon="mdi-close" variant="text" size="small" @click="cleanupDialog.show = false"></v-btn>
          </template>
        </v-card-item>
        
        <v-card-text class="py-6 text-center">
          <div class="text-subtitle-1 text-zinc-secondary px-2">
            This will only delete the indexed information and not the actual photos. Do you want to continue?
          </div>
        </v-card-text>

        <v-card-actions class="pa-4 bg-zinc-50 border-top-subtle ga-2">
          <v-btn variant="tonal" color="zinc-muted" @click="cleanupDialog.show = false" class="siegu-btn flex-grow-1" height="44">Cancel</v-btn>
          <v-btn variant="flat" color="black" @click="startConfirmedCleanup" class="siegu-btn flex-grow-1" height="44">Clear</v-btn>
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
          <v-card-title class="text-h6 text-zinc-primary font-weight-bold">Wipe & Remove?</v-card-title>
          <template v-slot:append>
            <v-btn icon="mdi-close" variant="text" size="small" @click="removeFolderDialog.show = false"></v-btn>
          </template>
        </v-card-item>
        
        <v-card-text class="py-6 text-center">
          <div class="text-subtitle-1 text-zinc-secondary px-2">
            This will remove the folder reference and <strong>permanently delete all indexed AI data</strong> for these files. Your actual photos will not be touched.
          </div>
        </v-card-text>

        <v-card-actions class="pa-4 bg-zinc-50 border-top-subtle ga-2">
          <v-btn variant="tonal" color="zinc-muted" @click="removeFolderDialog.show = false" class="siegu-btn flex-grow-1" height="44">Cancel</v-btn>
          <v-btn variant="flat" color="black" @click="startConfirmedRemoveFolder" class="siegu-btn flex-grow-1" height="44">Wipe Data</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <FolderPicker
        v-model="showFolderPicker"
        @select="onFolderSelected"
    />
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
    bgSync: false,
    downloadedModels: [],
    selectedModels: [],
    downloadProgress: {},
    pendingCount: 0,
    globalEta: 0,
    unlistenEta: null,
    unlistenProgress: null,
    unlistenModelProgress: null,
    modelProgress: {}, // { model_id: { pending, total } }
    aiModels: [
      { id: 'clip', title: 'Smart Search', desc: 'Finds objects and scenes in your photos.', search: "Try searching: 'dog', 'beach', 'sunset'", size: '350MB' },
      { id: 'ultraface', title: 'Face Grouping', desc: 'Finds faces and groups them by person.', search: "Try searching: 'Mom', 'John'", size: '2MB' },
      { id: 'ocr', title: 'Text Finder', desc: 'Reads text inside photos like receipts.', search: "Try searching: 'Receipt', 'Invoice', 'Menu'", size: '20MB' },
      { id: 'nsfw', title: 'Safe Mode', desc: 'Detects and hides sensitive content.', search: "Helps keep your library family-friendly.", size: '80MB' },
      { id: 'aesthetics', title: 'Quality Scorer', desc: 'Rates photos by how good they look.', search: "Helps you find your best shots quickly.", size: '80MB' },
      { id: 'blip', title: 'Photo Describer', desc: 'Writes sentences about what is in photos.', search: "Try searching: 'Family eating outside'", size: '950MB' },
      { id: 'yolo', title: 'Object Pro', desc: 'Pinpoint accuracy for finding specific items.', search: "Adds deep tags for 80+ common objects.", size: '15MB' },
      { id: 'arcface', title: 'Face Pro', desc: 'Advanced recognition for better grouping.', search: "drastically improves face matching accuracy.", size: '120MB' },
      { id: 'midas', title: 'Depth Vision', desc: 'Analyzes 3D depth in landscapes.', search: "Adds depth-based filters to your library.", size: '60MB' },
      { id: 'whisper', title: 'Audio Search', desc: 'Transcribes words spoken in videos.', search: "Search for things people said in videos.", size: '150MB' },
    ],
    logs: [],
    performance: {
      scanThreads: 4,
      indexingMode: "immediate",
    },
    maxThreads: 8,
    indexingModes: [
      { title: "Immediate", value: "immediate" },
      { title: "Idle Only", value: "idle" },
      { title: "Manual Only", value: "manual" },
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
    }
  }),
  computed: {
    missingSelectedCount() {
      return this.selectedModels.filter(id => !this.downloadedModels.includes(id)).length;
    }
  },
  async mounted() {
    listen("log-message", (event) => {
      const log = {
        time: new Date().toLocaleTimeString(),
        message: event.payload,
        type: event.payload.toLowerCase().includes("error") ? "error" : "info"
      };
      this.logs.unshift(log);
      if (this.logs.length > 100) this.logs.pop();
    });

    listen("download-progress", (event) => {
        const { model, downloaded, total } = event.payload;
        this.downloadProgress = { ...this.downloadProgress, [model]: { downloaded, total } };
    });

    listen("download-complete", () => {
        this.isDownloading = false;
        this.checkExistingModels();
        this.$emit('models-ready');
    });

    this.unlistenProgress = await listen("indexing-progress", (event) => {
        this.pendingCount = event.payload;
    });

    this.unlistenEta = await listen("indexing-eta", (event) => {
        this.globalEta = event.payload;
    });

    this.unlistenModelProgress = await listen("model-progress", (event) => {
        const { model, pending, total } = event.payload;
        this.modelProgress = { 
          ...this.modelProgress, 
          [model]: { 
            pending, 
            total: total || (this.modelProgress[model]?.total || 0) 
          } 
        };
    });

    this.dataDir = await homeDir();
    this.isAndroid = (await platform()) === 'android';
    await this.checkExistingModels();
    await this.loadPerformanceConfig();

    const bgSyncVal = await invoke("get_config", { key: "bg_sync" });
    this.bgSync = bgSyncVal === "true";
    this.fetchLogs();
    this.list_directories();
  },
  methods: {
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
    },
    getModeLabel(val) {
      return this.indexingModes.find(m => m.value === val)?.title || val;
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
    async savePerformanceConfig() {
      await invoke("save_config", { key: "scan_threads", value: this.performance.scanThreads.toString() });
    },
    async setIndexingMode(mode) {
      this.performance.indexingMode = mode;
      await invoke("save_config", { key: "indexing_mode", value: mode });
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
      try {
        await invoke("analyze_model", { modelId });
      } catch (err) {
        console.error("Failed to start model analysis", err);
      }
    },
    isModelProcessing(modelId) {
      const progress = this.modelProgress[modelId];
      return progress && progress.pending > 0;
    },
    getModelPending(modelId) {
      return this.modelProgress[modelId]?.pending || 0;
    },
    isModelDownloading(modelId) {
      if (modelId === 'clip') return this.downloadProgress['clip-visual'] || this.downloadProgress['clip-text'] || this.downloadProgress['clip-tokenizer'];
      if (modelId === 'ocr') return this.downloadProgress['ocr-det'] || this.downloadProgress['ocr-rec'] || this.downloadProgress['ocr-dict'];
      return !!this.downloadProgress[modelId];
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
      } catch (err) {}
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
      invoke("remove_directory", { path }).then(() => { this.list_directories(); });
    },
    onFolderSelected(path) {
      invoke("add_directory", { path }).then(() => {
        this.list_directories();
      });
    }
  },
  beforeUnmount() {
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
</style>
