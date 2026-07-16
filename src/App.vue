<script>
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import DeviceList from './components/DeviceList.vue';
import Map from './components/Map.vue';
import Photos from './components/Photos.vue';
import People from './components/People.vue';
import Setting from './components/Setting.vue';
import Greet from './components/Greet.vue';
import Connect from './components/Connect.vue';
import FolderPicker from './components/FolderPicker.vue';
import GuidedTour from './components/GuidedTour.vue';
import logo from './assets/logo.png';

export default {
  components: {
    DeviceList,
    Map,
    Photos,
    People,
    Setting,
    Greet,
    Connect,
    FolderPicker,
    GuidedTour,
  },
  data: () => ({
    logoUrl: logo,
    clean_install: false,
    scanning: false,
    scanStatus: 'idle',
    scanProgress: {
      current: 0,
      total: 0,
      progress: 0,
      current_directory: '',
    },
    indexingCount: 0,
    unindexedCount: 0,
    lastScanTime: 'Never',
    search: '',
    objects: [],
    recentSearches: [],
    searchDisabledDialog: { show: false },
    faces: [],
    filters: {
      favoritesOnly: false,
      videosOnly: false,
      dateRange: 'all',
      folder: null,
    },
    directories: [],
    syncPath: '',
    showSyncPicker: false,
    current_page: 'home',
    downloadProgress: {},
    isDownloadingModels: false,
    onboardingStep: 'greet',
    showTour: false,
    downloadedModels: [],
    deviceConnected: false,
    connectionMode: 'host',
    showConnectUI: false,
    os: '',
    syncStatus: {
      status: 'idle',
      progress: 0,
      type: 'idle',
    },
    syncError: {
      show: false,
      message: '',
    },
    currentScanFile: {
      current: 0,
      total: 0,
      filename: '',
      eta_secs: 0,
    },
    currentAiJob: {
      id: '',
      filename: '',
      model: '',
    },
    modelProgress: {},
  }),
  beforeMount() {
    this.applyTheme();
  },
  async mounted() {
    invoke('get_os').then((os) => (this.os = os));

    // Debug helpers — call from DevTools console:
    //   $scan()       — manually start scan
    //   $photos()     — list first 20 photos in DB
    //   $status()     — show current scan/DB state
    window.$scan = () =>
      invoke('scan_files')
        .then(() => console.log('scan_files invoked'))
        .catch((e) => console.error('scan_files ERROR:', e));
    window.$photos = async () => {
      const r = await invoke('list_files', {
        offset: 0,
        limit: 20,
        query: '',
        scan: false,
        favoritesOnly: false,
        videosOnly: false,
      });
      const data = JSON.parse(r);
      console.log(`📸 ${data.length} photos in DB (first 20):`, data);
      return data;
    };
    window.$status = async () => {
      const p = await window.$photos();
      console.log(
        'scanStatus:',
        this.scanStatus,
        'scanning:',
        this.scanning,
        'isActive:',
        this.isActive,
      );
    };

    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    mq.addEventListener('change', this.applyTheme);
    window.addEventListener('siegu-theme-changed', this.applyTheme);
    window.addEventListener('storage', (e) => {
      if (e.key === 'siegu_theme') this.applyTheme();
    });

    invoke('get_people')
      .then((response) => {
        this.faces = JSON.parse(response);
      })
      .catch(() => {});

    listen('download-progress', (event) => {
      const { model, downloaded, total } = event.payload;
      this.isDownloadingModels = true;
      this.downloadProgress = { ...this.downloadProgress, [model]: { downloaded, total } };
    });

    listen('download-complete', () => {
      // Check if all selected models are done (simplified: just clear after a delay)
      setTimeout(() => {
        this.isDownloadingModels = false;
        this.downloadProgress = {};
      }, 2000);
    });

    const initialized = await invoke('is_initialized');
    this.clean_install = !initialized;

    invoke('get_last_scan_time').then((time) => {
      if (time !== 'Never') {
        const timestamp = parseInt(time);
        const date = new Date(timestamp * 1000);
        this.lastScanTime = date.toLocaleString(this.getLocale());
      }
    });

    invoke('get_indexing_status').then((count) => {
      this.indexingCount = this.normalizeIndexingCount(count);
    });

    invoke('get_unindexed_count').then((count) => {
      this.unindexedCount = this.normalizeIndexingCount(count);
    });

    listen('indexing-progress', (event) => {
      const count = this.normalizeIndexingCount(event.payload);
      this.indexingCount = count;
      this.unindexedCount = count;
    });

    invoke('get_people').then((response) => {
      try {
        const parsed = JSON.parse(response);
        this.faces = Array.isArray(parsed) ? parsed : [];
      } catch (e) {
        console.error('Failed to parse people:', e);
      }
    });

    listen('log-message', (event) => {
      console.log('[rust]', event.payload);
    });

    listen('scan-progress', (event) => {
      console.log('scan-progress', event.payload);
      const data = event.payload;
      this.scanStatus = data.status;
      if (data.status === 'discovering') {
        this.scanning = true;
        this.scanProgress = {
          current: data.current || 0,
          total: data.total || 0,
          progress: data.progress || 0,
          current_directory: data.current_directory || '',
        };
      } else if (data.status === 'indexing') {
        this.scanning = true;
      } else if (data.status === 'complete') {
        this.scanning = false;
        this.currentScanFile = { current: 0, total: 0, filename: '', eta_secs: 0 };
        this.currentAiJob = { id: '', filename: '', model: '' };
        this.lastScanTime = new Date().toLocaleString(this.getLocale());
        setTimeout(() => {
          this.scanStatus = 'idle';
        }, 3000);
      }
    });

    listen('file-scan-progress', (event) => {
      const data = event.payload;
      this.currentScanFile = {
        current: data.current || 0,
        total: data.total || 0,
        filename: data.filename || '',
        eta_secs: data.eta_secs || 0,
      };
    });

    listen('current-ai-job', (event) => {
      const data = event.payload;
      this.currentAiJob = {
        id: data.id || '',
        filename: data.filename || '',
        model: data.model || '',
      };
    });

    listen('model-progress', (event) => {
      const data = event.payload;
      this.modelProgress = { ...this.modelProgress, [data.model]: data };
    });

    listen('sync-progress', (event) => {
      this.syncStatus = {
        status: event.payload.status,
        progress: event.payload.progress,
      };

      if (event.payload.status === 'Up to date' || event.payload.status.startsWith('Finished')) {
        setTimeout(() => {
          if (this.syncStatus.status === event.payload.status) {
            this.syncStatus.status = 'idle';
            this.current_page = 'home';
          }
        }, 2000);
      }
    });

    this.list_directories();
    listen('start-sync', () => {
      console.log('Peer requested sync start.');
      this.finishSetupAndScan();
    });

    listen('photo-scanned', (event) => {
      // Local scan: show indexing status if needed
    });

    listen('photo-received', (event) => {
      this.syncStatus = {
        status: this.$t('sync.received_memory', { id: event.payload.id }),
        progress: 100,
        type: 'received',
      };
      setTimeout(() => {
        if (this.syncStatus.type === 'received') {
          this.syncStatus.status = 'idle';
          this.syncStatus.type = 'idle';
        }
      }, 2000);
    });

    listen('photo-synced', (event) => {
      this.syncStatus = {
        status: this.$t('sync.synced_memory', { id: event.payload }),
        progress: 100,
        type: 'synced',
      };
      setTimeout(() => {
        if (this.syncStatus.type === 'synced') {
          this.syncStatus.status = 'idle';
          this.syncStatus.type = 'idle';
        }
      }, 2000);
    });

    listen('sync-error', (event) => {
      this.syncError = {
        show: true,
        message: event.payload,
      };
      this.syncStatus.status = this.$t('sync.error_status');
    });

    this.checkModels();

    invoke('get_top_tags').then((response) => {
      try {
        const parsed = JSON.parse(response);
        this.objects = Array.isArray(parsed) ? parsed : [];
      } catch (e) {}
    });
  },
  computed: {
    isMobile() {
      return this.os === 'android' || this.os === 'ios';
    },
    hasActiveFilters() {
      return this.filters.favoritesOnly || this.filters.dateRange !== 'all' || this.filters.folder;
    },
    isActive() {
      return (
        this.scanStatus === 'discovering' ||
        this.scanStatus === 'indexing' ||
        this.indexingCount > 0 ||
        this.unindexedCount > 0
      );
    },
    statusLabel() {
      if (this.scanStatus === 'discovering') return this.$t('sync.scanning');
      if (this.scanStatus === 'indexing' || this.indexingCount > 0) return this.$t('sync.indexing');
      if (this.unindexedCount > 0) return this.$t('sync.indexing');
      return this.$t('sync.refresh');
    },
    searchItems() {
      return this.objects;
    },
    recentSearchItems() {
      return this.recentSearches.slice(0, 5).map((s) => ({
        title: s,
        type: 'recent',
      }));
    },
    searchHelpText() {
      if (!this.searchItems.length && !this.filteredPeople.length) {
        return this.$t('search.no_data');
      }
      return '';
    },
    filteredPeople() {
      if (!this.faces) return [];
      if (!this.search) return this.faces.slice(0, 10);
      const q = this.search.toLowerCase();
      return this.faces.filter((p) => p.name.toLowerCase().includes(q)).slice(0, 10);
    },
  },
  methods: {
    normalizeIndexingCount(value) {
      const count = Number(value);
      if (!Number.isSafeInteger(count) || count < 0 || count > 1000000) return 0;
      return count;
    },
    getLocale() {
      return localStorage.getItem('siegu_language') || 'en';
    },
    formatIndexingCount(value) {
      return this.normalizeIndexingCount(value).toLocaleString(this.getLocale());
    },
    resetFilters() {
      this.filters = { favoritesOnly: false, videosOnly: false, dateRange: 'all', folder: null };
    },
    list_directories() {
      invoke('list_directories').then((response) => {
        this.directories = JSON.parse(response);
      });
    },
    scan: async function () {
      this.scanStatus = 'discovering';
      this.scanning = true;
      await invoke('scan_files');
    },
    list_objects: function (val) {
      if (val && val.length > 0) {
        invoke('list_objects', { query: val })
          .then((response) => {
            this.objects = JSON.parse(response);
          })
          .catch((e) => {
            console.error('list_objects failed:', e);
          });
      } else {
        invoke('get_top_tags')
          .then((response) => {
            this.objects = JSON.parse(response);
          })
          .catch((e) => {
            console.error('get_top_tags failed:', e);
          });
      }
    },
    getFaceImageSrc(crop_path, encoded) {
      return encoded || '';
    },
    iconForType(type) {
      if (type === 'person') return 'mdi-account';
      if (type === 'location') return 'mdi-map-marker';
      if (type === 'date') return 'mdi-calendar-month';
      return 'mdi-tag';
    },
    iconColor(type) {
      if (type === 'person') return '#0ea5e9';
      if (type === 'location') return '#f59e0b';
      if (type === 'date') return '#8b5cf6';
      return '#10b981';
    },
    onSearchUpdate(val) {
      if (typeof val === 'string' && val) {
        this.current_page = 'home';
      }
    },
    onSearchClick(e) {
      // No-op: the search input handles its own focus.
    },
    onSearchSelect(val) {
      if (val) {
        const title = typeof val === 'object' ? val.title : val;
        this.search = title;
        this.addRecentSearch(title);
      }
    },
    addPersonToSearch(person) {
      this.search = person.name;
      this.addRecentSearch(person.name);
      this.current_page = 'home';
    },
    addRecentSearch(val) {
      if (!val) return;
      const arr = this.recentSearches.filter((s) => s !== val);
      arr.unshift(val);
      this.recentSearches = arr.slice(0, 10);
    },
    async setSyncPath(path) {
      try {
        this.syncPath = path;
        await invoke('save_config', { key: 'sync_path', value: path });
        await invoke('initialize_sync_folder', { path: path });
        this.list_directories();
      } catch (err) {
        this.syncError = {
          show: true,
          message: this.$t('sync.folder_init_error', { error: err }),
        };
      }
    },
    async triggerSync() {
      await invoke('request_start_sync');
      this.current_page = 'home';
      this.clean_install = false;
      this.onboardingStep = 'complete';
      setTimeout(() => {
        this.showTour = true;
      }, 800);
    },
    async checkModels() {
      const downloaded = await invoke('check_models');
      this.downloadedModels = downloaded;
    },
    formatEta(secs) {
      if (secs < 60) return `${secs}s`;
      const mins = Math.floor(secs / 60);
      const remainSecs = secs % 60;
      return `${mins}m ${remainSecs}s`;
    },
    async finishSetupAndScan() {
      this.clean_install = false;
      this.onboardingStep = 'complete';
      this.current_page = 'home';

      if (this.deviceConnected) {
        console.log('Device linked during onboarding, skipping local scan.');
        return;
      }

      // Show guided tour after scan starts
      setTimeout(() => {
        this.showTour = true;
      }, 1500);

      // Start scan and show progress immediately
      setTimeout(() => {
        this.scan();
      }, 500);
    },
    applyTheme() {
      const pref = localStorage.getItem('siegu_theme') || 'system';
      let theme;
      if (pref === 'system') {
        theme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
      } else {
        theme = pref;
      }
      this.$vuetify.theme.global.name = theme;
    },
  },
  beforeUnmount() {
    window
      .matchMedia('(prefers-color-scheme: dark)')
      .removeEventListener('change', this.applyTheme);
    window.removeEventListener('siegu-theme-changed', this.applyTheme);
  },
  watch: {
    search(val) {
      this.list_objects(val);
    },
  },
};
</script>

<template>
  <v-app class="bg-siegu-main">
    <!-- Global Download Bar -->
    <v-system-bar
      v-if="isDownloadingModels"
      color="black"
      theme="dark"
      class="justify-center py-1"
      height="auto"
    >
      <div class="d-flex align-center py-1">
        <v-progress-circular
          indeterminate
          size="14"
          width="2"
          class="mr-2"
          color="white"
        ></v-progress-circular>
        <span class="text-caption font-weight-bold">{{ $t('settings.downloading_models') }}</span>
      </div>
    </v-system-bar>

    <!-- Guided Onboarding -->
    <template v-if="clean_install">
      <Greet
        v-if="onboardingStep === 'greet'"
        @setup-local="onboardingStep = 'folders'"
        @setup-sync="
          onboardingStep = 'sync';
          connectionMode = 'join';
          showConnectUI = true;
        "
      ></Greet>

      <!-- Step 2: Folders -->
      <v-container v-else-if="onboardingStep === 'folders'" class="fill-height" fluid>
        <v-row justify="center">
          <v-col cols="12" sm="10" md="8" lg="6">
            <v-card variant="flat" rounded="xl" class="pa-8 border-subtle">
              <div class="text-center mb-8">
                <div class="siegu-icon-circle mx-auto mb-4">
                  <v-icon color="white">mdi-folder-plus</v-icon>
                </div>
                <h2 class="text-h4 font-weight-bold text-zinc-primary">
                  {{ $t('onboarding.add_media_title') }}
                </h2>
                <p class="text-zinc-secondary">{{ $t('onboarding.add_media_desc') }}</p>
              </div>

              <Setting :embedded="true" hide-ai-section @folder-added="list_directories" />

              <v-btn
                block
                color="black"
                height="56"
                class="siegu-btn mt-8"
                :disabled="directories.length === 0"
                @click="onboardingStep = 'models'"
              >
                {{ $t('onboarding.continue_ai') }}
              </v-btn>
            </v-card>
          </v-col>
        </v-row>
      </v-container>

      <!-- Step 3: Models -->
      <v-container v-else-if="onboardingStep === 'models'" class="fill-height" fluid>
        <v-row justify="center">
          <v-col cols="12" sm="10" md="8" lg="6">
            <v-card variant="flat" rounded="xl" class="pa-8 border-subtle">
              <div class="text-center mb-8">
                <div class="siegu-icon-circle-dark mx-auto mb-4">
                  <v-icon color="white">mdi-auto-fix</v-icon>
                </div>
                <h2 class="text-h4 font-weight-bold text-zinc-primary">
                  {{ $t('onboarding.ai_title') }}
                </h2>
                <p class="text-zinc-secondary">{{ $t('onboarding.ai_desc') }}</p>
              </div>

              <v-alert
                v-if="!isDownloadingModels && downloadedModels.length < 2"
                border="start"
                color="zinc-50"
                class="border-subtle mb-6"
              >
                <template v-slot:prepend>
                  <v-icon color="zinc-primary">mdi-information-outline</v-icon>
                </template>
                <div class="text-caption text-zinc-secondary">
                  {{ $t('onboarding.model_info') }}
                </div>
              </v-alert>

              <Setting :embedded="true" hide-folder-section @models-ready="checkModels" />

              <v-btn
                block
                color="black"
                height="56"
                class="siegu-btn mt-8"
                :loading="isDownloadingModels"
                :disabled="downloadedModels.length < 2 && !isDownloadingModels"
                @click="onboardingStep = 'sync'"
              >
                {{
                  downloadedModels.length < 2
                    ? $t('onboarding.download_required')
                    : $t('onboarding.continue')
                }}
              </v-btn>
            </v-card>
          </v-col>
        </v-row>
      </v-container>

      <!-- Step 4: Sync & Devices (Skippable) -->
      <v-container v-else-if="onboardingStep === 'sync'" class="fill-height" fluid>
        <v-row justify="center">
          <v-col cols="12" sm="10" md="8" lg="6">
            <v-card variant="flat" rounded="xl" class="pa-8 border-subtle">
              <div class="text-center mb-8">
                <div class="siegu-icon-circle mx-auto mb-4">
                  <v-icon color="white">mdi-cellphone-link</v-icon>
                </div>
                <h2 class="text-h4 font-weight-bold text-zinc-primary">
                  {{ $t('onboarding.sync_title') }}
                </h2>
                <p class="text-zinc-secondary">{{ $t('onboarding.sync_desc') }}</p>
              </div>

              <!-- Sync Path Selection (Only for Guest/Join mode) -->
              <v-expand-transition>
                <div v-if="connectionMode === 'join'" class="mb-8">
                  <div
                    class="text-caption font-weight-bold text-zinc-muted mb-4 tracking-widest uppercase text-center"
                  >
                    {{ $t('onboarding.target_sync_folder') }}
                  </div>
                  <v-card
                    variant="flat"
                    class="bg-zinc-50 border-subtle pa-4 rounded-xl d-flex align-center"
                  >
                    <v-icon color="zinc-secondary" class="mr-3">mdi-folder-sync</v-icon>
                    <div class="flex-grow-1 overflow-hidden">
                      <div class="text-caption text-zinc-secondary">
                        {{ $t('onboarding.sync_storage') }}
                      </div>
                      <div class="text-body-2 font-weight-bold text-zinc-primary text-truncate">
                        {{ syncPath || $t('onboarding.auto_select') }}
                      </div>
                    </div>
                    <v-btn
                      variant="flat"
                      size="small"
                      color="black"
                      class="siegu-btn-sm ml-4"
                      @click="showSyncPicker = true"
                    >
                      {{ $t('onboarding.change') }}
                    </v-btn>
                  </v-card>
                </div>
              </v-expand-transition>

              <FolderPicker v-model="showSyncPicker" @select="setSyncPath" />

              <div class="d-flex justify-center mb-8">
                <Connect
                  :embedded="true"
                  :initial-mode="connectionMode"
                  :hide-mode-toggle="true"
                  @connected="deviceConnected = true"
                  @mode-change="connectionMode = $event"
                />
              </div>

              <v-fade-transition>
                <div v-if="deviceConnected" class="mb-6">
                  <div
                    class="bg-success-light border-success pa-4 rounded-xl mb-4 text-center d-flex align-center justify-center"
                  >
                    <v-icon color="success" class="mr-2">mdi-check-circle</v-icon>
                    <span class="text-success font-weight-bold">{{
                      $t('onboarding.device_linked')
                    }}</span>
                  </div>

                  <!-- Real-time Sync Progress Card -->
                  <v-card
                    v-if="syncStatus.status !== 'idle'"
                    variant="flat"
                    class="bg-zinc-50 border-subtle pa-4 rounded-xl"
                  >
                    <div class="d-flex align-center mb-2">
                      <v-progress-circular
                        indeterminate
                        size="16"
                        width="2"
                        color="black"
                        class="mr-2"
                      ></v-progress-circular>
                      <div class="text-caption font-weight-bold text-zinc-primary">
                        {{ syncStatus.status }}
                      </div>
                      <v-spacer></v-spacer>
                      <div class="text-caption text-zinc-muted">
                        {{ Math.round(syncStatus.progress) }}%
                      </div>
                    </div>
                    <v-progress-linear
                      :model-value="syncStatus.progress"
                      color="black"
                      height="6"
                      rounded
                    ></v-progress-linear>
                  </v-card>
                </div>
              </v-fade-transition>

              <div class="d-flex flex-column ga-3">
                <template v-if="!showConnectUI">
                  <v-btn
                    block
                    color="black"
                    height="56"
                    class="siegu-btn"
                    @click="showConnectUI = true"
                  >
                    <v-icon start class="mr-2">mdi-link-variant</v-icon>
                    {{ $t('onboarding.link_device') }}
                  </v-btn>
                  <v-btn
                    block
                    color="black"
                    height="56"
                    class="siegu-btn"
                    @click="onboardingStep = 'finalize'"
                  >
                    {{ $t('onboarding.skip') }}
                  </v-btn>
                </template>
                <template v-else>
                  <v-btn
                    v-if="deviceConnected"
                    block
                    color="black"
                    height="56"
                    class="siegu-btn"
                    @click="triggerSync"
                  >
                    <v-icon start class="mr-2">mdi-sync</v-icon>
                    {{ $t('onboarding.start_syncing') }}
                  </v-btn>
                  <v-btn
                    v-else
                    block
                    color="black"
                    height="56"
                    class="siegu-btn"
                    @click="onboardingStep = 'finalize'"
                  >
                    {{ $t('onboarding.skip') }}
                  </v-btn>
                </template>
              </div>
            </v-card>
          </v-col>
        </v-row>
      </v-container>

      <!-- Step 5: Finalize & Scan -->
      <v-container v-else-if="onboardingStep === 'finalize'" class="fill-height" fluid>
        <v-row justify="center">
          <v-col cols="12" sm="10" md="8" lg="6">
            <v-card variant="flat" rounded="xl" class="pa-8 border-subtle text-center">
              <div class="success-check-animation mb-8">
                <v-icon size="80" color="success">mdi-check-decagram</v-icon>
              </div>
              <h2 class="text-h3 font-weight-black text-zinc-primary mb-4">
                {{ $t('onboarding.ready_title') }}
              </h2>
              <p class="text-body-1 text-zinc-secondary mb-10">
                {{ $t('onboarding.ready_desc') }}
              </p>

              <v-btn
                block
                color="black"
                height="64"
                class="siegu-btn mb-4"
                @click="finishSetupAndScan"
              >
                <v-icon start class="mr-2">{{
                  deviceConnected ? 'mdi-sync' : 'mdi-magnify-scan'
                }}</v-icon>
                {{ deviceConnected ? $t('onboarding.finish_setup') : $t('onboarding.start_scan') }}
              </v-btn>

              <div class="text-caption text-zinc-muted">
                {{ $t('onboarding.scan_desc') }}
              </div>
            </v-card>
          </v-col>
        </v-row>
      </v-container>
    </template>

    <v-layout v-else class="bg-siegu-main">
      <v-app-bar
        elevation="0"
        v-if="current_page === 'home'"
        color="surface"
        class="border-bottom-subtle px-2"
      >
        <v-row class="px-2 align-center no-gutters">
          <v-col cols="auto">
            <v-menu offset-y transition="scale-transition">
              <template v-slot:activator="{ props }">
                <v-btn
                  v-bind="props"
                  color="#000000"
                  theme="dark"
                  variant="flat"
                  :class="isMobile ? 'px-2' : 'px-4'"
                  height="40"
                  rounded="lg"
                  data-tour="scan-button"
                >
                  <div class="d-flex align-center">
                    <div :class="isMobile ? '' : 'mr-2'">
                      <v-progress-circular
                        v-if="isActive"
                        indeterminate
                        size="16"
                        width="2"
                        color="white"
                      ></v-progress-circular>
                      <v-icon v-else size="18" color="white">mdi-sync</v-icon>
                    </div>
                    <span v-if="!isMobile" class="text-white font-weight-bold">{{
                      statusLabel
                    }}</span>
                  </div>
                </v-btn>
              </template>
              <v-card
                min-width="320"
                border
                class="mt-2 border-subtle overflow-hidden"
                color="surface"
                rounded="xl"
              >
                <div class="bg-zinc-50 pa-4 border-bottom-subtle">
                  <div class="text-overline font-weight-black text-zinc-muted mb-1">
                    {{ $t('sync.status_label') }}
                  </div>
                  <div class="d-flex align-center justify-space-between">
                    <div class="text-subtitle-1 font-weight-bold text-zinc-primary">
                      {{ $t('app.name') }} {{ $t('app.sync') }}
                    </div>
                    <v-chip
                      v-if="isActive"
                      size="x-small"
                      color="black"
                      variant="flat"
                      class="text-white"
                      >{{ statusLabel }}</v-chip
                    >
                  </div>
                </div>

                <v-card-text class="pa-4">
                  <v-list density="compact" bg-color="transparent" class="pa-0">
                    <v-list-item class="px-0 mb-4">
                      <template v-slot:prepend>
                        <v-icon color="zinc-muted" class="mr-3">mdi-folder-outline</v-icon>
                      </template>
                      <v-list-item-title class="text-zinc-primary font-weight-bold">{{
                        $t('sync.file_scanner')
                      }}</v-list-item-title>
                      <v-list-item-subtitle class="text-zinc-secondary">
                        {{
                          scanStatus === 'discovering'
                            ? $t('sync.processing_folder', {
                                current: scanProgress.current,
                                total: scanProgress.total,
                              })
                            : scanStatus === 'indexing'
                              ? $t('sync.switched_ai')
                              : $t('sync.idle')
                        }}
                      </v-list-item-subtitle>

                      <div v-if="scanStatus === 'discovering'" class="mt-2">
                        <div
                          v-if="currentScanFile.total > 0"
                          class="d-flex align-center justify-space-between mb-1"
                        >
                          <span
                            class="text-caption text-zinc-muted text-truncate"
                            style="max-width: 200px"
                            >{{ currentScanFile.filename || $t('sync.scanning') }}</span
                          >
                          <span class="text-caption text-zinc-muted ml-2"
                            >{{ currentScanFile.current }}/{{ currentScanFile.total }}</span
                          >
                        </div>
                        <v-progress-linear
                          v-if="currentScanFile.total > 0"
                          :model-value="(currentScanFile.current / currentScanFile.total) * 100"
                          color="black"
                          height="4"
                          rounded
                        ></v-progress-linear>
                        <div
                          v-if="currentScanFile.eta_secs > 0"
                          class="text-caption text-zinc-muted mt-1"
                        >
                          {{
                            $t('sync.time_remaining', { time: formatEta(currentScanFile.eta_secs) })
                          }}
                        </div>
                      </div>

                      <div v-if="scanStatus === 'indexing'" class="mt-2">
                        <v-progress-linear
                          indeterminate
                          color="black"
                          height="4"
                          rounded
                        ></v-progress-linear>
                      </div>
                    </v-list-item>

                    <v-list-item class="px-0 mb-4">
                      <template v-slot:prepend>
                        <v-icon color="zinc-muted" class="mr-3">mdi-auto-fix</v-icon>
                      </template>
                      <v-list-item-title class="text-zinc-primary font-weight-bold">{{
                        $t('sync.ai_intelligence')
                      }}</v-list-item-title>
                      <v-list-item-subtitle class="text-zinc-secondary">
                        {{
                          indexingCount > 0
                            ? $t('sync.jobs_remaining', {
                                count: formatIndexingCount(indexingCount),
                              })
                            : scanStatus === 'indexing'
                              ? $t('sync.finalizing')
                              : $t('sync.all_indexed')
                        }}
                      </v-list-item-subtitle>
                      <div
                        v-if="currentAiJob.filename && indexingCount > 0"
                        class="mt-2 bg-zinc-50 rounded-lg pa-2 border-subtle"
                      >
                        <div class="d-flex align-center">
                          <v-progress-circular
                            indeterminate
                            size="12"
                            width="2"
                            color="black"
                            class="mr-2"
                          ></v-progress-circular>
                          <div
                            class="text-caption text-zinc-primary text-truncate"
                            style="max-width: 220px"
                          >
                            {{ currentAiJob.filename }}
                          </div>
                        </div>
                        <div
                          v-if="currentAiJob.model"
                          class="text-caption text-zinc-muted mt-1 ml-5"
                        >
                          {{ $t('sync.model_label', { model: currentAiJob.model }) }}
                        </div>
                      </div>
                      <div v-if="Object.keys(modelProgress).length > 0" class="mt-2">
                        <div
                          v-for="(mp, key) in modelProgress"
                          :key="key"
                          class="d-flex align-center justify-space-between mb-1"
                        >
                          <span class="text-caption text-zinc-muted text-capitalize">{{
                            key
                          }}</span>
                          <v-chip
                            :color="
                              mp.status === 'completed'
                                ? 'success'
                                : mp.status === 'error'
                                  ? 'error'
                                  : 'default'
                            "
                            size="x-small"
                            variant="flat"
                            class="text-white"
                            density="compact"
                          >
                            {{
                              mp.status === 'completed'
                                ? $t('settings.ready')
                                : mp.status === 'running'
                                  ? `${mp.pending} ${$t('sync.jobs_left', { count: mp.pending })}`
                                  : mp.status
                            }}
                          </v-chip>
                        </div>
                      </div>
                    </v-list-item>
                  </v-list>

                  <v-divider class="my-4 border-subtle"></v-divider>

                  <div class="d-flex align-center justify-space-between mb-6">
                    <span class="text-caption text-zinc-muted">{{
                      $t('sync.last_sync', { time: lastScanTime })
                    }}</span>
                  </div>

                  <v-btn
                    v-if="!isActive"
                    @click="scan()"
                    variant="flat"
                    color="black"
                    block
                    height="56"
                    class="siegu-btn"
                  >
                    <div class="d-flex align-center">
                      <div class="siegu-icon-circle mr-3">
                        <v-icon>mdi-sync</v-icon>
                      </div>
                      <div class="text-left">
                        <div class="font-weight-bold">{{ $t('sync.sync_library') }}</div>
                        <div
                          class="text-caption text-zinc-muted"
                          style="font-size: 10px; opacity: 0.7"
                        >
                          {{ $t('sync.refresh_files') }}
                        </div>
                      </div>
                    </div>
                  </v-btn>
                  <div v-else class="text-center py-2">
                    <v-progress-circular
                      indeterminate
                      color="black"
                      size="24"
                    ></v-progress-circular>
                    <div class="text-caption mt-2 text-zinc-muted">
                      {{ $t('sync.processing_bg') }}
                    </div>
                  </div>
                </v-card-text>
              </v-card>
            </v-menu>
          </v-col>

          <v-col class="mx-2 flex-grow-1">
            <div class="search-wrapper" @click="onSearchClick">
              <v-autocomplete
                v-model:search="search"
                :items="searchItems"
                item-title="title"
                item-value="title"
                prepend-inner-icon="mdi-magnify"
                variant="solo-filled"
                density="compact"
                :placeholder="$t('search.placeholder')"
                hide-details
                flat
                rounded="lg"
                class="search-autocomplete w-100"
                data-tour="search"
                bg-color="rgb(var(--v-theme-surface))"
                :disabled="false"
                :menu-props="{ contentClass: 'siegu-list', elevation: 4 }"
                :no-data-text="searchHelpText"
                :filter="() => true"
                @update:search="onSearchUpdate"
                @update:model-value="onSearchSelect"
              >
                <template v-slot:item="{ props, item }">
                  <v-list-item v-bind="props" :title="item.raw.title">
                    <template v-slot:prepend>
                      <v-icon size="18" class="mr-2" :color="iconColor(item.raw.type)">
                        {{ iconForType(item.raw.type) }}
                      </v-icon>
                    </template>
                  </v-list-item>
                </template>
                <template v-slot:append-inner>
                  <v-tooltip location="bottom" max-width="280">
                    <template v-slot:activator="{ props }">
                      <v-icon v-bind="props" size="18" color="#a1a1aa" class="cursor-pointer"
                        >mdi-help-circle-outline</v-icon
                      >
                    </template>
                    <div class="pa-2">
                      <div class="text-caption font-weight-bold mb-2">
                        {{ $t('search.help_title') }}
                      </div>
                      <div class="text-caption mb-1">{{ $t('search.help_desc') }}</div>
                      <div class="text-caption mb-1">&#8226; {{ $t('search.help_tags') }}</div>
                      <div class="text-caption mb-1">&#8226; {{ $t('search.help_people') }}</div>
                      <div class="text-caption mb-1">&#8226; {{ $t('search.help_location') }}</div>
                      <div class="text-caption mb-1">&#8226; {{ $t('search.help_date') }}</div>
                      <div class="text-caption mb-1">&#8226; {{ $t('search.help_caption') }}</div>
                      <div class="text-caption">&#8226; {{ $t('search.help_ocr') }}</div>
                    </div>
                  </v-tooltip>
                </template>
                <template v-slot:prepend-item>
                  <div v-if="!search && recentSearches.length > 0">
                    <v-list-subheader
                      class="text-zinc-muted text-uppercase tracking-widest text-caption py-2"
                      >{{ $t('search.recent') }}</v-list-subheader
                    >
                    <div class="pa-2 d-flex flex-column ga-1">
                      <div
                        v-for="(term, i) in recentSearches.slice(0, 5)"
                        :key="i"
                        class="d-flex align-center cursor-pointer px-2 py-1 rounded-lg recent-item"
                        @click="
                          search = term;
                          addRecentSearch(term);
                          current_page = 'home';
                        "
                      >
                        <v-icon size="16" color="#a1a1aa" class="mr-2">mdi-history</v-icon>
                        <span class="text-caption text-zinc-primary">{{ term }}</span>
                      </div>
                    </div>
                    <v-divider class="border-subtle my-1"></v-divider>
                  </div>
                  <div v-if="filteredPeople.length > 0">
                    <v-list-subheader
                      class="text-zinc-muted text-uppercase tracking-widest text-caption py-2"
                      >{{ $t('search.people_section') }}</v-list-subheader
                    >
                    <div class="pa-2 d-flex flex-nowrap overflow-x-auto ga-2">
                      <div
                        v-for="person in filteredPeople"
                        :key="person.id"
                        class="d-flex flex-column align-center cursor-pointer min-w-60"
                        @click="addPersonToSearch(person)"
                      >
                        <v-avatar size="40" class="mb-1 border-subtle">
                          <v-img
                            :src="getFaceImageSrc(person.representative_crop, person.encoded)"
                          ></v-img>
                        </v-avatar>
                        <span
                          class="text-caption text-zinc-muted text-truncate w-100 text-center"
                          >{{ person.name }}</span
                        >
                      </div>
                    </div>
                    <v-divider class="border-subtle my-1"></v-divider>
                  </div>
                  <v-list-subheader
                    v-if="!search"
                    class="text-zinc-muted text-uppercase tracking-widest text-caption py-2"
                    >{{ $t('search.top_suggestions') }}</v-list-subheader
                  >
                </template>
              </v-autocomplete>
            </div>
          </v-col>

          <v-col cols="auto">
            <v-menu :close-on-content-click="false" offset-y>
              <template v-slot:activator="{ props }">
                <v-btn icon size="small" variant="text" v-bind="props" color="#18181b">
                  <v-badge :model-value="hasActiveFilters" color="black" dot px="1">
                    <v-icon size="20">mdi-filter-variant</v-icon>
                  </v-badge>
                </v-btn>
              </template>
              <v-card
                min-width="250"
                border
                class="mt-2 border-subtle"
                color="surface"
                rounded="xl"
              >
                <v-list bg-color="transparent" density="compact" class="px-2 ga-2">
                  <v-list-item class="px-0">
                    <v-switch
                      v-model="filters.favoritesOnly"
                      :label="$t('filters.favorites_only')"
                      color="#000000"
                      hide-details
                      density="compact"
                      inset
                      class="text-zinc-secondary px-2"
                    ></v-switch>
                  </v-list-item>
                  <v-list-item class="px-0">
                    <v-switch
                      v-model="filters.videosOnly"
                      :label="$t('filters.videos_only')"
                      color="#000000"
                      hide-details
                      density="compact"
                      inset
                      class="text-zinc-secondary px-2"
                    ></v-switch>
                  </v-list-item>
                  <v-divider class="border-subtle my-2"></v-divider>
                  <v-list-subheader
                    class="text-zinc-muted text-uppercase tracking-widest text-caption px-0"
                    >{{ $t('filters.date_range') }}</v-list-subheader
                  >
                  <v-list-item class="px-0">
                    <v-btn-toggle
                      v-model="filters.dateRange"
                      mandatory
                      variant="flat"
                      density="compact"
                      class="ga-2 w-100 bg-transparent"
                    >
                      <v-btn value="all" size="x-small" class="siegu-btn flex-grow-1">{{
                        $t('filters.all')
                      }}</v-btn>
                      <v-btn value="month" size="x-small" class="siegu-btn flex-grow-1">{{
                        $t('filters.month')
                      }}</v-btn>
                      <v-btn value="year" size="x-small" class="siegu-btn flex-grow-1">{{
                        $t('filters.year')
                      }}</v-btn>
                    </v-btn-toggle>
                  </v-list-item>
                  <v-divider class="border-subtle my-2"></v-divider>
                  <v-list-subheader
                    class="text-zinc-muted text-uppercase tracking-widest text-caption px-0"
                    >{{ $t('filters.folder') }}</v-list-subheader
                  >
                  <v-list-item class="px-0">
                    <v-select
                      v-model="filters.folder"
                      :items="directories"
                      :placeholder="$t('filters.all_folders')"
                      variant="solo-filled"
                      density="compact"
                      hide-details
                      flat
                      rounded="lg"
                      class="siegu-field"
                    ></v-select>
                  </v-list-item>
                </v-list>
                <v-card-actions class="pa-4">
                  <v-btn variant="flat" class="siegu-btn w-100 py-4" @click="resetFilters">
                    <div class="d-flex align-center">
                      <div class="siegu-icon-circle siegu-icon-circle-sm mr-2">
                        <v-icon size="12">mdi-refresh</v-icon>
                      </div>
                      <span>{{ $t('filters.reset') }}</span>
                    </div>
                  </v-btn>
                </v-card-actions>
              </v-card>
            </v-menu>
          </v-col>
        </v-row>
      </v-app-bar>

      <v-main class="bg-siegu-main">
        <!-- Persistent Progress Banner -->
        <v-slide-y-reverse-transition>
          <div v-if="isActive" class="progress-banner" data-tour="scan-progress">
            <div class="progress-banner-inner px-4 py-2">
              <div class="d-flex align-center justify-space-between flex-wrap ga-2">
                <div
                  class="d-flex align-center ga-2 min-width-0 flex-shrink-1"
                  style="max-width: 55%"
                >
                  <v-progress-circular
                    indeterminate
                    size="16"
                    width="2"
                    color="black"
                  ></v-progress-circular>
                  <div class="text-caption font-weight-bold text-zinc-primary text-truncate">
                    <template v-if="scanStatus === 'discovering'">
                      <span>{{ $t('sync.discovering') }}</span>
                      <span
                        v-if="currentScanFile.filename"
                        class="text-zinc-muted font-weight-regular"
                        >{{ currentScanFile.filename }}</span
                      >
                      <span v-else class="text-zinc-muted font-weight-regular">{{
                        scanProgress.current_directory
                          ? scanProgress.current_directory.split('/').pop()
                          : '...'
                      }}</span>
                    </template>
                    <template v-else-if="scanStatus === 'indexing' || indexingCount > 0">
                      <span>{{ $t('sync.indexing') }}: </span>
                      <span
                        v-if="currentAiJob.filename"
                        class="text-zinc-muted font-weight-regular"
                        >{{ currentAiJob.filename }}</span
                      >
                      <span v-else class="text-zinc-muted font-weight-regular">{{
                        $t('sync.jobs_left', { count: formatIndexingCount(indexingCount) })
                      }}</span>
                    </template>
                  </div>
                </div>
                <div class="d-flex align-center ga-3 flex-shrink-0">
                  <span
                    v-if="scanStatus === 'discovering' && currentScanFile.total > 0"
                    class="text-caption text-zinc-muted"
                  >
                    {{ currentScanFile.current }}/{{ currentScanFile.total }}
                    <span v-if="currentScanFile.eta_secs > 0">
                      · ~{{ formatEta(currentScanFile.eta_secs) }}</span
                    >
                  </span>
                  <span v-else-if="indexingCount > 0" class="text-caption text-zinc-muted">
                    {{ $t('sync.jobs_left', { count: formatIndexingCount(indexingCount) }) }}
                  </span>
                  <v-progress-linear
                    v-if="scanStatus === 'discovering' && currentScanFile.total > 0"
                    :model-value="(currentScanFile.current / currentScanFile.total) * 100"
                    color="black"
                    height="4"
                    rounded
                    max-width="120"
                    class="progress-bar-mini"
                  ></v-progress-linear>
                </div>
              </div>
            </div>
          </div>
        </v-slide-y-reverse-transition>
        <div data-tour="photos" class="w-100">
          <Photos
            v-if="current_page === 'home'"
            :search-query="search"
            :filters="filters"
            @clear-search="search = ''"
          />
        </div>
        <People v-if="current_page === 'people'" @search-person="addPersonToSearch" />
        <Map v-if="current_page === 'location'" />
        <DeviceList v-if="current_page === 'devices'" />
        <Setting v-if="current_page === 'settings'" @done="current_page = 'home'" />
      </v-main>

      <div class="dock-container" v-if="!clean_install">
        <v-sheet
          class="dock d-flex justify-space-around align-center pa-2 rounded-pill mb-8"
          elevation="0"
          width="100%"
          max-width="380"
          color="surface"
        >
          <v-btn
            icon
            variant="text"
            @click="current_page = 'home'"
            size="small"
            class="siegu-dock-btn"
            :class="{ 'siegu-dock-btn--active': current_page === 'home' }"
            data-tour="dock-home"
          >
            <v-img
              :src="logoUrl"
              width="24"
              height="24"
              :class="current_page === 'home' ? 'opacity-100' : 'opacity-40'"
            ></v-img>
          </v-btn>
          <v-btn
            icon
            variant="text"
            @click="current_page = 'people'"
            size="small"
            class="siegu-dock-btn"
            :class="{ 'siegu-dock-btn--active': current_page === 'people' }"
            data-tour="dock-people"
          >
            <v-icon size="24">mdi-account-group-outline</v-icon>
          </v-btn>
          <v-btn
            icon
            variant="text"
            @click="current_page = 'location'"
            size="small"
            class="siegu-dock-btn"
            :class="{ 'siegu-dock-btn--active': current_page === 'location' }"
            data-tour="dock-map"
          >
            <v-icon size="24">mdi-map-outline</v-icon>
          </v-btn>
          <v-btn
            icon
            variant="text"
            @click="current_page = 'devices'"
            size="small"
            class="siegu-dock-btn"
            :class="{ 'siegu-dock-btn--active': current_page === 'devices' }"
            data-tour="dock-devices"
          >
            <v-icon size="24">mdi-laptop</v-icon>
          </v-btn>
          <v-btn
            icon
            variant="text"
            @click="current_page = 'settings'"
            size="small"
            class="siegu-dock-btn"
            :class="{ 'siegu-dock-btn--active': current_page === 'settings' }"
            data-tour="dock-settings"
          >
            <v-icon size="24">mdi-cog-outline</v-icon>
          </v-btn>
        </v-sheet>
      </div>
    </v-layout>

    <!-- Persistent Sync Status Banner (Non-blocking) -->
    <v-fade-transition>
      <div
        v-if="syncStatus.status !== 'idle' && syncStatus.status !== 'Up to date' && !clean_install"
        class="sync-banner-container"
      >
        <v-sheet
          class="sync-banner d-flex align-center px-4 py-2 rounded-pill shadow-xl border-subtle"
          color="surface"
        >
          <v-progress-circular
            v-if="syncStatus.progress === 0 || syncStatus.progress === 100"
            indeterminate
            size="18"
            width="2"
            color="black"
            class="mr-3"
          ></v-progress-circular>
          <div v-else class="mr-3 d-flex align-center">
            <v-progress-circular
              :model-value="syncStatus.progress"
              size="22"
              width="3"
              color="black"
            >
              <span style="font-size: 8px; font-weight: bold">{{
                Math.round(syncStatus.progress)
              }}</span>
            </v-progress-circular>
          </div>
          <div
            class="text-caption font-weight-bold text-zinc-primary text-truncate pr-2"
            style="max-width: 220px"
          >
            {{ syncStatus.status }}
          </div>
          <v-divider vertical class="mx-2 opacity-10" length="16"></v-divider>
          <v-btn
            icon="mdi-close"
            variant="text"
            size="x-small"
            class="text-zinc-muted"
            @click="syncStatus.status = 'idle'"
          ></v-btn>
        </v-sheet>
      </div>
    </v-fade-transition>

    <v-snackbar v-model="syncError.show" color="error" rounded="lg" elevation="12">
      <div class="d-flex align-center">
        <v-icon class="mr-3">mdi-alert-circle</v-icon>
        <div class="text-subtitle-2 font-weight-bold">{{ syncError.message }}</div>
      </div>
      <template v-slot:actions>
        <v-btn variant="text" @click="syncError.show = false">{{ $t('common.close') }}</v-btn>
      </template>
    </v-snackbar>

    <GuidedTour :active="showTour" @finish="showTour = false" @skip="showTour = false" />

    <v-dialog v-model="searchDisabledDialog.show" max-width="400" rounded="xl">
      <v-card color="surface" border class="border-subtle overflow-hidden">
        <v-card-item class="bg-zinc-100 py-4">
          <template v-slot:prepend>
            <div class="siegu-icon-circle-dark mr-3">
              <v-icon color="#ffffff" size="small">mdi-information-outline</v-icon>
            </div>
          </template>
          <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
            $t('search.disabled_title')
          }}</v-card-title>
          <template v-slot:append>
            <v-btn
              icon="mdi-close"
              variant="text"
              size="small"
              @click="searchDisabledDialog.show = false"
            ></v-btn>
          </template>
        </v-card-item>
        <v-card-text class="py-6 text-center">
          <div class="text-subtitle-1 text-zinc-secondary px-2">
            {{ $t('search.disabled_desc') }}
          </div>
        </v-card-text>
        <v-card-actions class="pa-4 bg-zinc-50 border-top-subtle ga-2">
          <v-btn
            variant="flat"
            color="black"
            @click="searchDisabledDialog.show = false"
            class="siegu-btn flex-grow-1"
            height="44"
            >{{ $t('search.disabled_got_it') }}</v-btn
          >
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-app>
</template>

<style scoped>
.dock-container {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  display: flex;
  justify-content: center;
  pointer-events: none;
  z-index: 2000;
}

.dock {
  pointer-events: auto;
  backdrop-filter: blur(16px);
  border: 1px solid var(--color-border-default);
}

.sync-banner-container {
  position: fixed;
  bottom: 100px; /* Above the dock */
  left: 0;
  right: 0;
  display: flex;
  justify-content: center;
  z-index: 3000; /* Above everything */
  pointer-events: none;
}

.sync-banner {
  pointer-events: auto;
  min-width: 240px;
  max-width: 90vw;
  box-shadow: 0 10px 30px -5px rgba(0, 0, 0, 0.15) !important;
  border: 1px solid var(--color-border-subtle) !important;
}

.siegu-dock-btn {
  color: var(--color-text-muted) !important;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1) !important;
  border-radius: 50% !important;
}

.siegu-dock-btn:hover {
  background: var(--color-bg-hover) !important;
  color: var(--color-text-primary) !important;
  transform: translateY(-2px);
}

.siegu-dock-btn--active {
  color: var(--color-text-primary) !important;
  background: var(--color-bg-hover) !important;
}

.progress-banner {
  position: sticky;
  top: 0;
  z-index: 100;
  background: var(--color-bg-primary);
  border-bottom: 1px solid var(--color-border-subtle);
}

.progress-banner-inner {
  max-width: 1200px;
  margin: 0 auto;
}

.progress-bar-mini {
  min-width: 80px;
}

.min-width-0 {
  min-width: 0;
}
.search-wrapper {
  width: 100%;
  cursor: text;
}
.recent-item:hover {
  background: var(--color-bg-hover);
}
</style>
