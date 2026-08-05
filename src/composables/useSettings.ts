import { ref, reactive, computed, onUnmounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { platform } from '@tauri-apps/plugin-os';
import { listen } from '@tauri-apps/api/event';
import { check } from '@tauri-apps/plugin-updater';
import { AI_MODEL_IDS } from '@/types/models';
import type {
  DirectoryEntry,
  LogEntry,
  DownloadProgressState,
  ModelProgressState,
  PerformanceConfig,
  DownloadDialogState,
  CleanupDialogState,
  RemoveFolderDialogState,
  SnackbarState,
  IndexingMode,
  PerformancePreset,
  UpdateStatus,
} from '@/types/settings';
import type { Update } from '@tauri-apps/plugin-updater';
import { DEFAULT_SIGNALING_URL, appendToken, pingSignalling } from '@/services/signalling';
import type { PingResult } from '@/services/signalling';

let listenersSetUp = false;
const cleanupFns: Array<() => void> = [];

function detectPlatformFromUserAgent(): string {
  const ua = typeof navigator !== 'undefined' ? navigator.userAgent : '';
  if (/android/i.test(ua)) return 'android';
  if (/iphone|ipad|ipod/i.test(ua)) return 'ios';
  if (/mac/i.test(ua)) return 'macos';
  if (/win/i.test(ua)) return 'windows';
  if (/linux/i.test(ua)) return 'linux';
  return '';
}

function detectPlatform(): string {
  try {
    const detected = platform();
    if (detected) return detected;
  } catch {
    // plugin unavailable — fall back to user agent
  }
  return detectPlatformFromUserAgent();
}

export function useSettings() {
  const { t } = useI18n();

  const directories = ref<DirectoryEntry[]>([]);
  const showFolderPicker = ref(false);
  const isAndroid = ref(false);
  const currentPlatform = ref(detectPlatformFromUserAgent());

  const downloadedModels = ref<string[]>([]);
  const selectedModels = ref<string[]>([...AI_MODEL_IDS]);
  const downloadProgress = ref<Record<string, DownloadProgressState>>({});
  const downloadingModels = ref<Record<string, boolean>>({});
  const isDownloading = ref(false);

  const pendingCount = ref(0);
  const globalEta = ref(0);
  const activeModelId = ref<string | null>(null);
  const activeModelHoldUntil = ref(0);
  const modelProgress = ref<Record<string, ModelProgressState>>({});

  const modelEnabled = ref<Record<string, boolean>>({});

  const performance = reactive<PerformanceConfig>({
    scanThreads: 4,
    mlThreads: 2,
    batchDelayMs: 0,
    memoryBudgetMb: 0,
    indexingMode: 'immediate',
  });
  const maxThreads = 8;

  const modelsLoaded = ref(false);
  const isMemoryFreeing = ref(false);

  const PRESETS = {
    low: { scanThreads: 2, mlThreads: 1, batchDelayMs: 200 },
    balanced: { scanThreads: 4, mlThreads: 2, batchDelayMs: 0 },
    full: { scanThreads: maxThreads, mlThreads: 4, batchDelayMs: 0 },
  } as const;

  const currentPreset = computed<PerformancePreset>(() => {
    const values = {
      scanThreads: performance.scanThreads,
      mlThreads: performance.mlThreads,
      batchDelayMs: performance.batchDelayMs,
    };
    for (const name of ['low', 'balanced', 'full'] as const) {
      if (
        values.scanThreads === PRESETS[name].scanThreads &&
        values.mlThreads === PRESETS[name].mlThreads &&
        values.batchDelayMs === PRESETS[name].batchDelayMs
      ) {
        return name;
      }
    }
    return 'custom';
  });

  const logs = ref<LogEntry[]>([]);

  const snackbar = reactive<SnackbarState>({ show: false, text: '', error: false });
  const downloadDialog = reactive<DownloadDialogState>({
    show: false,
    title: '',
    message: '',
    models: [],
  });
  const cleanupDialog = reactive<CleanupDialogState>({ show: false });
  const removeFolderDialog = reactive<RemoveFolderDialogState>({ show: false, path: '' });

  const isCleaning = ref(false);
  const updateStatus = ref<UpdateStatus>('idle');
  const updateInfo = ref<Update | null>(null);

  const signalingUrl = ref('');
  const signalingToken = ref('');
  const signalingTesting = ref(false);
  const signalingPingResult = ref<PingResult | null>(null);

  const uiNow = ref(Date.now());

  let uiClock: ReturnType<typeof setInterval> | null = null;
  const MAX_LOG_ENTRIES = 100;

  function startClock(): void {
    if (uiClock) return;
    uiClock = window.setInterval(() => {
      uiNow.value = Date.now();
    }, 1000);
  }

  async function setupEventListeners(): Promise<void> {
    if (listenersSetUp) return;
    listenersSetUp = true;

    cleanupFns.push(
      await listen<string>('log-message', (event) => {
        const log: LogEntry = {
          time: new Date().toLocaleTimeString(localStorage.getItem('siegu_language') || 'en'),
          message: event.payload,
          type: event.payload.toLowerCase().includes('error') ? 'error' : 'info',
        };
        logs.value.unshift(log);
        if (logs.value.length > MAX_LOG_ENTRIES) logs.value.pop();
      }),
    );

    cleanupFns.push(
      await listen<{ model: string; downloaded: number; total: number }>(
        'download-progress',
        (event) => {
          const { model, downloaded, total } = event.payload;
          downloadProgress.value = { ...downloadProgress.value, [model]: { downloaded, total } };
        },
      ),
    );

    cleanupFns.push(
      await listen<void>('download-complete', () => {
        isDownloading.value = false;
        downloadingModels.value = {};
        downloadProgress.value = {};
        checkExistingModels();
      }),
    );

    cleanupFns.push(
      await listen<number | string>('indexing-progress', (event) => {
        pendingCount.value = normalizeIndexingCount(event.payload);
      }),
    );

    cleanupFns.push(
      await listen<number>('indexing-eta', (event) => {
        globalEta.value = event.payload;
      }),
    );

    cleanupFns.push(
      await listen<{
        model: string;
        pending: number | null;
        total: number | null;
        status: string;
        message: string;
      }>('model-progress', (event) => {
        const { model, pending, total, status, message } = event.payload;
        const previous = modelProgress.value[model] || ({} as ModelProgressState);
        const normalizedPending = typeof pending === 'number' ? pending : previous.pending;
        const normalizedTotal = typeof total === 'number' ? total : previous.total;
        const normalizedStatus = (status ||
          (normalizedPending && normalizedPending > 0
            ? 'running'
            : 'idle')) as ModelProgressState['status'];
        activeModelId.value = model;
        if (['completed', 'up_to_date', 'unavailable', 'error'].includes(normalizedStatus)) {
          activeModelHoldUntil.value = Date.now() + 15000;
        } else {
          activeModelHoldUntil.value = 0;
        }
        modelProgress.value = {
          ...modelProgress.value,
          [model]: {
            ...previous,
            pending: normalizedPending,
            total: normalizedTotal,
            status: normalizedStatus,
            message: message || previous.message || '',
            updatedAt: Date.now(),
          },
        };
      }),
    );
  }

  onUnmounted(() => {
    if (uiClock) window.clearInterval(uiClock);
  });

  async function init(): Promise<void> {
    startClock();
    await setupEventListeners();
    const detectedPlatform = detectPlatform();
    isAndroid.value = detectedPlatform === 'android';
    currentPlatform.value = detectedPlatform;
    await checkExistingModels();
    await loadPerformanceConfig();
    await loadModelEnabledStates();
    await loadModelsLoaded();
    await loadSignallingConfig();
    await fetchLogs();
    await listDirectories();
  }

  async function listDirectories(): Promise<void> {
    try {
      const response = await invoke<string>('list_directories');
      const dirs = JSON.parse(response) as string[];
      console.log('[Settings] listDirectories result:', dirs);
      directories.value = dirs.map((dir) => ({ title: dir.split('/').pop() || dir, value: dir }));
    } catch (e) {
      console.error('[Settings] listDirectories failed:', e);
      directories.value = [];
    }
  }

  async function selectDirectory(): Promise<void> {
    if (isAndroid.value) {
      showFolderPicker.value = true;
      return;
    }
    try {
      console.log('[Settings] Opening directory dialog...');
      const selection = await open({ multiple: true, directory: true });
      console.log('[Settings] Dialog result:', JSON.stringify(selection));
      if (Array.isArray(selection)) {
        for (const path of selection) {
          console.log('[Settings] Adding directory:', path);
          await invoke('add_directory', { path });
        }
      } else if (selection) {
        console.log('[Settings] Adding directory:', selection);
        await invoke('add_directory', { path: selection });
      } else {
        console.log('[Settings] Dialog cancelled or empty result');
      }
      await listDirectories();
    } catch (e) {
      console.error('[Settings] selectDirectory failed:', e);
    }
  }

  async function removeDirectory(path: string): Promise<void> {
    try {
      await invoke('remove_directory', { path });
      await listDirectories();
    } catch {
      // silent
    }
  }

  function openRemoveFolderFull(path: string): void {
    removeFolderDialog.path = path;
    removeFolderDialog.show = true;
  }

  async function startConfirmedRemoveFolder(): Promise<void> {
    const path = removeFolderDialog.path;
    removeFolderDialog.show = false;
    try {
      await invoke('abort_indexing');
      await new Promise((r) => setTimeout(r, 300));
      await invoke('remove_directory_full', { path });
      await listDirectories();
    } catch {
      // silent
    }
  }

  function onFolderSelected(path: string): void {
    invoke('add_directory', { path }).then(() => {
      listDirectories();
    });
  }

  function showSnackbar(text: string, error = false): void {
    snackbar.show = true;
    snackbar.text = text;
    snackbar.error = error;
  }

  function setLanguage(code: string): void {
    localStorage.setItem('siegu_language', code);
    window.location.reload();
  }

  function setAppearance(val: string): void {
    localStorage.setItem('siegu_theme', val);
  }

  async function fetchLogs(): Promise<void> {
    try {
      const logsStr = await invoke<string>('get_logs', { limit: 100 });
      const parsed = JSON.parse(logsStr) as Array<{
        timestamp: string;
        message: string;
        level: string;
      }>;
      logs.value = parsed.map((l) => ({
        time: new Date(l.timestamp).toLocaleTimeString(
          localStorage.getItem('siegu_language') || 'en',
        ),
        message: l.message,
        type: l.level === 'error' ? 'error' : 'info',
      }));
    } catch {
      // silent
    }
  }

  async function clearLogs(): Promise<void> {
    await invoke('clear_logs');
    logs.value = [];
    showSnackbar('Logs cleared');
  }

  function normalizeIndexingCount(value: number | string): number {
    const count = Number(value);
    if (!Number.isSafeInteger(count) || count < 0 || count > 1000000) return 0;
    return count;
  }

  function formatIndexingCount(value: number): string {
    return normalizeIndexingCount(value).toLocaleString(
      localStorage.getItem('siegu_language') || 'en',
    );
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

  function getModeLabel(val: string): string {
    return val;
  }

  async function checkExistingModels(): Promise<void> {
    try {
      const downloaded = await invoke<string[]>('check_models');
      downloadedModels.value = downloaded;
      selectedModels.value = [...AI_MODEL_IDS];
    } catch {
      downloadedModels.value = [];
    }
  }

  function configKeyForModel(modelId: string): string {
    return 'model_enabled_' + modelId;
  }

  function configKeysForModel(modelId: string): string[] {
    if (modelId === 'face') {
      return ['model_enabled_face', 'model_enabled_arcface'];
    }
    return ['model_enabled_' + modelId];
  }

  async function loadModelEnabledStates(): Promise<void> {
    try {
      const configStr = await invoke<string>('get_config');
      const config = JSON.parse(configStr) as Record<string, string>;
      const enabled: Record<string, boolean> = {};
      for (const id of AI_MODEL_IDS) {
        const key = configKeyForModel(id);
        enabled[id] = config[key] !== 'false';
      }
      modelEnabled.value = enabled;
    } catch {
      // silent
    }
  }

  async function toggleModel(modelId: string): Promise<void> {
    modelEnabled.value[modelId] = !modelEnabled.value[modelId];
    const keys = configKeysForModel(modelId);
    for (const key of keys) {
      await invoke('save_config', { key, value: modelEnabled.value[modelId] ? 'true' : 'false' });
    }
  }

  async function loadPerformanceConfig(): Promise<void> {
    try {
      const configStr = await invoke<string>('get_config');
      const config = JSON.parse(configStr) as Record<string, string>;
      if (config.scan_threads) {
        const val = parseInt(config.scan_threads);
        if (!isNaN(val)) performance.scanThreads = val;
      }
      if (config.ml_threads) {
        const val = parseInt(config.ml_threads);
        if (!isNaN(val)) performance.mlThreads = val;
      }
      if (config.batch_delay_ms) {
        const val = parseInt(config.batch_delay_ms);
        if (!isNaN(val)) performance.batchDelayMs = val;
      }
      if (config.ml_memory_budget_mb) {
        const val = parseInt(config.ml_memory_budget_mb);
        if (!isNaN(val)) performance.memoryBudgetMb = val;
      }
      if (config.indexing_mode) {
        performance.indexingMode = config.indexing_mode as IndexingMode;
      }
    } catch {
      // silent
    }
  }

  async function savePerformanceConfig(): Promise<void> {
    const entries: Array<[string, string]> = [
      ['scan_threads', performance.scanThreads.toString()],
      ['ml_threads', performance.mlThreads.toString()],
      ['batch_delay_ms', performance.batchDelayMs.toString()],
      ['ml_memory_budget_mb', performance.memoryBudgetMb.toString()],
    ];
    for (const [key, value] of entries) {
      await invoke('save_config', { key, value });
    }
  }

  async function applyPreset(preset: Exclude<PerformancePreset, 'custom'>): Promise<void> {
    const values = PRESETS[preset];
    performance.scanThreads = values.scanThreads;
    performance.mlThreads = values.mlThreads;
    performance.batchDelayMs = values.batchDelayMs;
    await savePerformanceConfig();
    showSnackbar(t('settings.preset_applied', { name: t('settings.preset_' + preset) }));
  }

  async function loadModelsLoaded(): Promise<void> {
    try {
      modelsLoaded.value = await invoke<boolean>('get_models_loaded');
    } catch {
      modelsLoaded.value = false;
    }
  }

  async function freeMemory(): Promise<void> {
    if (isMemoryFreeing.value) return;
    isMemoryFreeing.value = true;
    try {
      await invoke('unload_models');
      modelsLoaded.value = false;
      showSnackbar(t('settings.memory_freed'));
    } catch (e) {
      showSnackbar(String(e), true);
    } finally {
      isMemoryFreeing.value = false;
    }
  }

  async function setIndexingMode(mode: string): Promise<void> {
    performance.indexingMode = mode as IndexingMode;
    await invoke('save_config', { key: 'indexing_mode', value: mode });
    if (mode === 'manual') {
      await invoke('abort_indexing');
      showSnackbar('Manual indexing enabled');
    } else {
      showSnackbar(`Indexing mode set to ${mode}`);
    }
  }

  async function loadSignallingConfig(): Promise<void> {
    try {
      const config = JSON.parse(await invoke<string>('get_config')) as Record<string, string>;
      signalingUrl.value = config.signaling_url || '';
      signalingToken.value = config.signaling_token || '';
    } catch {
      signalingUrl.value = '';
      signalingToken.value = '';
    }
  }

  function effectiveSignalingUrl(): string {
    const base = signalingUrl.value.trim() || DEFAULT_SIGNALING_URL;
    return appendToken(base, signalingToken.value);
  }

  async function saveSignallingConfig(): Promise<void> {
    const base = signalingUrl.value.trim();
    try {
      await invoke('save_config', { key: 'signaling_url', value: base });
      await invoke('save_config', { key: 'signaling_token', value: signalingToken.value.trim() });
      signalingPingResult.value = null;
      showSnackbar(base ? t('settings.signalling_saved') : t('settings.signalling_reset'));
    } catch (e) {
      showSnackbar(t('settings.signalling_save_failed', { error: e }), true);
    }
  }

  async function testSignalling(): Promise<void> {
    if (signalingTesting.value) return;
    signalingTesting.value = true;
    signalingPingResult.value = null;
    const url = effectiveSignalingUrl();
    try {
      signalingPingResult.value = await pingSignalling(url);
    } catch (e) {
      signalingPingResult.value = {
        ok: false,
        message: t('settings.signalling_ping_failed', { error: e }),
      };
    } finally {
      signalingTesting.value = false;
    }
  }

  async function downloadModels(
    forceUpdate = false,
    specificModels: string[] | null = null,
  ): Promise<void> {
    let modelsToDownload = specificModels || selectedModels.value;
    if (!forceUpdate && !specificModels) {
      modelsToDownload = AI_MODEL_IDS.filter((m) => !downloadedModels.value.includes(m));
    }
    if (!modelsToDownload || modelsToDownload.length === 0) return;

    isDownloading.value = true;
    downloadingModels.value = {
      ...downloadingModels.value,
      ...Object.fromEntries(modelsToDownload.map((m) => [m, true])),
    };
    modelsToDownload.forEach((m) => {
      downloadProgress.value[m] = { downloaded: 0, total: 1 };
    });

    try {
      await invoke('download_models', { models: modelsToDownload });
    } catch (error) {
      isDownloading.value = false;
      modelsToDownload.forEach((m) => {
        delete downloadingModels.value[m];
      });
      downloadingModels.value = { ...downloadingModels.value };
      const message = error instanceof Error ? error.message : String(error);
      showSnackbar(message || t('settings.download_failed'), true);
    }
  }

  function getProgress(model: string): number {
    const progress = downloadProgress.value[model];
    if (!progress || !progress.total) return downloadedModels.value.includes(model) ? 100 : 0;
    return (progress.downloaded / progress.total) * 100;
  }

  function isModelProcessing(modelId: string): boolean {
    const progress = modelProgress.value[modelId];
    return (
      !!progress &&
      (progress.status === 'starting' || (progress.pending != null && progress.pending > 0))
    );
  }

  function isModelActive(modelId: string): boolean {
    return (
      activeModelId.value === modelId &&
      (isModelProcessing(modelId) || uiNow.value < activeModelHoldUntil.value)
    );
  }

  function getModelProgressPercent(modelId: string): number {
    const progress = modelProgress.value[modelId];
    if (!progress || !progress.total) return 0;
    const pending = Math.max(progress.pending || 0, 0);
    return Math.max(0, Math.min(100, ((progress.total - pending) / progress.total) * 100));
  }

  function getModelProgressText(modelId: string): string {
    const progress = modelProgress.value[modelId];
    if (!progress || progress.status === 'starting') return 'Starting';
    if (!progress.total) return `${progress.pending || 0} left`;
    return `${Math.max(progress.total - (progress.pending || 0), 0)} of ${progress.total}`;
  }

  function getModelStatusLabel(modelId: string): string {
    const progress = modelProgress.value[modelId];
    if (progress?.status === 'starting') return 'Starting';
    if (progress?.status === 'completed') return 'Finished';
    if (progress?.status === 'up_to_date') return 'Up to date';
    if (progress?.status === 'unavailable') return 'Unavailable';
    if (progress?.status === 'error') return 'Error';
    return 'Running';
  }

  function getModelStatusText(modelId: string): string {
    const progress = modelProgress.value[modelId];
    if (isModelProcessing(modelId)) return getModelProgressText(modelId);
    if (progress?.message) return progress.message;
    if (progress?.status === 'completed') return 'Finished';
    if (progress?.status === 'up_to_date') return 'Up to date';
    if (progress?.status === 'unavailable') return 'Not available';
    if (progress?.status === 'error') return 'Error';
    if (progress?.total != null && progress.total > 0 && progress.pending === 0) return 'Finished';
    if (progress?.total === 0 && progress.pending === 0) return 'Up to date';
    if (downloadedModels.value.includes(modelId)) return '';
    return 'Not downloaded';
  }

  function getModelActivityIcon(modelId: string): string {
    const status = modelProgress.value[modelId]?.status;
    if (status === 'completed' || status === 'up_to_date') return 'mdi-check-circle-outline';
    if (status === 'unavailable') return 'mdi-alert-circle-outline';
    if (status === 'error') return 'mdi-alert-outline';
    return 'mdi-robot-outline';
  }

  function isModelDownloading(modelId: string): boolean {
    return !!downloadingModels.value[modelId];
  }

  async function runModel(modelId: string): Promise<void> {
    const previous = modelProgress.value[modelId] || ({} as ModelProgressState);
    activeModelId.value = modelId;
    activeModelHoldUntil.value = 0;
    modelProgress.value = {
      ...modelProgress.value,
      [modelId]: {
        ...previous,
        pending: previous.pending || null,
        total: previous.total || null,
        status: 'starting',
        updatedAt: Date.now(),
      },
    };
    try {
      await invoke('analyze_model', { modelId });
      modelsLoaded.value = true;
    } catch {
      modelProgress.value = {
        ...modelProgress.value,
        [modelId]: {
          ...modelProgress.value[modelId],
          pending: 0,
          status: 'idle',
          updatedAt: Date.now(),
        },
      };
    }
  }

  async function startConfirmedCleanup(): Promise<void> {
    cleanupDialog.show = false;
    isCleaning.value = true;
    try {
      await invoke('abort_indexing');
      await new Promise((r) => setTimeout(r, 500));
      await invoke('cleanup_database', { confirm: true });
      window.location.reload();
    } catch {
      // silent
    } finally {
      isCleaning.value = false;
    }
  }

  async function checkUpdate(): Promise<void> {
    if (!updateSupported.value) return;
    updateStatus.value = 'checking';
    try {
      const update = await check();
      if (update) {
        updateInfo.value = update;
        updateStatus.value = 'available';
      } else {
        updateStatus.value = 'uptodate';
      }
    } catch {
      updateStatus.value = 'error';
    }
  }

  async function downloadUpdate(): Promise<void> {
    if (!updateSupported.value) return;
    if (!updateInfo.value) return;
    updateStatus.value = 'downloading';
    try {
      await updateInfo.value.downloadAndInstall();
      updateStatus.value = 'uptodate';
    } catch {
      updateStatus.value = 'error';
    }
  }

  const sortedModels = computed(() => {
    return [...AI_MODEL_IDS]
      .map((id) => ({ id, size: MODEL_SIZES[id] }))
      .sort((a, b) => {
        const aDownloaded = downloadedModels.value.includes(a.id);
        const bDownloaded = downloadedModels.value.includes(b.id);
        const aActive = isModelActive(a.id);
        const bActive = isModelActive(b.id);
        if (aActive && !bActive) return -1;
        if (!aActive && bActive) return 1;
        if (aDownloaded && !bDownloaded) return -1;
        if (!aDownloaded && bDownloaded) return 1;
        return 0;
      });
  });

  const isAnyModelProcessing = computed(() => {
    return AI_MODEL_IDS.some((model) => isModelProcessing(model));
  });

  const missingSelectedCount = computed(() => {
    return selectedModels.value.filter((id) => !downloadedModels.value.includes(id)).length;
  });

  function formatBytes(bytes: number): string {
    if (!bytes || bytes <= 0) return '0 MB';
    const gb = bytes / (1024 * 1024 * 1024);
    if (gb >= 1) return `${gb.toFixed(1)} GB`;
    return `${Math.round(bytes / (1024 * 1024))} MB`;
  }

  const modelRam = computed<Record<string, string>>(() => {
    const map: Record<string, string> = {};
    for (const id of AI_MODEL_IDS) {
      map[id] = formatBytes(MODEL_RAM_BYTES[id] || 0);
    }
    return map;
  });

  const totalModelRamEstimate = computed(() => {
    const bytes = AI_MODEL_IDS.reduce((sum, id) => {
      if (modelEnabled.value[id] && downloadedModels.value.includes(id)) {
        return sum + (MODEL_RAM_BYTES[id] || 0);
      }
      return sum;
    }, 0);
    return formatBytes(bytes);
  });

  const visibleActivityModel = computed(() => {
    if (!activeModelId.value) return null;
    const model = sortedModels.value.find((m) => m.id === activeModelId.value);
    if (!model) return null;
    if (isModelProcessing(model.id) || uiNow.value < activeModelHoldUntil.value) return model;
    return null;
  });

  const activeModelSummary = computed(() => {
    if (!visibleActivityModel.value) return '';
    return `${getModelStatusLabel(visibleActivityModel.value.id)}: ${t('models.' + visibleActivityModel.value.id + '.title')}`;
  });

  const updateSupported = computed(() => {
    return currentPlatform.value === 'windows' || currentPlatform.value === 'macos';
  });

  const updateStatusText = computed(() => {
    switch (updateStatus.value) {
      case 'checking':
        return 'Checking...';
      case 'available':
        return updateInfo.value?.version
          ? `Version ${updateInfo.value.version} available`
          : 'Update available';
      case 'uptodate':
        return 'Up to date';
      case 'downloading':
        return 'Downloading...';
      case 'error':
        return 'Update failed';
      default:
        return '';
    }
  });

  const updateBtnText = computed(() => {
    if (updateStatus.value === 'available') return 'Download update';
    return 'Check for updates';
  });

  const updateBtnIcon = computed(() => {
    if (updateStatus.value === 'available') return 'mdi-download';
    return 'mdi-update';
  });

  const availableLanguages = computed(() => {
    const codes = ['en', 'nl', 'fr', 'es', 'pap', 'de', 'it', 'pt'] as const;
    return codes.map((code) => ({
      code,
      label: code,
    }));
  });

  return {
    directories,
    showFolderPicker,
    isAndroid,
    currentPlatform,
    downloadedModels,
    selectedModels,
    downloadProgress,
    downloadingModels,
    isDownloading,
    pendingCount,
    globalEta,
    activeModelId,
    activeModelHoldUntil,
    modelProgress,
    modelEnabled,
    performance,
    maxThreads,
    modelsLoaded,
    isMemoryFreeing,
    currentPreset,
    modelRam,
    totalModelRamEstimate,
    logs,
    snackbar,
    downloadDialog,
    cleanupDialog,
    removeFolderDialog,
    isCleaning,
    updateStatus,
    updateInfo,
    uiNow,
    signalingUrl,
    signalingToken,
    signalingTesting,
    signalingPingResult,
    sortedModels,
    isAnyModelProcessing,
    missingSelectedCount,
    visibleActivityModel,
    activeModelSummary,
    updateSupported,
    updateStatusText,
    updateBtnText,
    updateBtnIcon,
    availableLanguages,
    init,
    listDirectories,
    selectDirectory,
    removeDirectory,
    openRemoveFolderFull,
    startConfirmedRemoveFolder,
    onFolderSelected,
    showSnackbar,
    setLanguage,
    setAppearance,
    fetchLogs,
    clearLogs,
    formatIndexingCount,
    formatEta,
    getModeLabel,
    checkExistingModels,
    toggleModel,
    loadPerformanceConfig,
    savePerformanceConfig,
    setIndexingMode,
    applyPreset,
    loadModelsLoaded,
    freeMemory,
    loadSignallingConfig,
    saveSignallingConfig,
    testSignalling,
    effectiveSignalingUrl,
    downloadModels,
    getProgress,
    isModelProcessing,
    isModelActive,
    getModelProgressPercent,
    getModelProgressText,
    getModelStatusLabel,
    getModelStatusText,
    getModelActivityIcon,
    isModelDownloading,
    runModel,
    startConfirmedCleanup,
    checkUpdate,
    downloadUpdate,
  };
}

const MODEL_SIZES: Record<string, string> = {
  clip: '350MB',
  face: '168MB',
  ocr: '20MB',
  nsfw: '328MB',
  aesthetics: '1.6GB',
  blip: '329MB',
  yolo: '15MB',
  midas: '508MB',
  whisper: '31MB',
};

// Estimated resident RAM per model, derived from the registry file sizes in
// crates/siegu-core/src/ml_engine/models.rs. Light models load twice (session
// pool size 2), so their RAM is roughly double their disk size.
const MODEL_RAM_BYTES: Record<string, number> = {
  clip: (351_685_709 + 254_058_553 + 2_224_119) * 2,
  face: 232_589 + 174_383_860,
  ocr: 2_423_224 + 8_967_018 + 190,
  nsfw: 343_401_688,
  aesthetics: 1_718_811_155,
  yolo: 12_823_574 * 2,
  blip: 345_122_738 + 647_427_238 + 711_396,
  midas: 533_061_339,
  whisper: 32_883_618 + 118_505_132,
};
