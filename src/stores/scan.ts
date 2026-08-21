import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import {
  getIndexingStatus,
  getUnindexedCount,
  abortIndexing,
  pauseIndexing,
  resumeIndexing,
} from '@/services/tauri';
import { listenEvent } from '@/services/events';
import { normalizeIndexingCount } from '@/composables/useMediaUtils';
import type { ScanStatus, ScanPhase, AnalysisActivity, ModelState } from '@/types/scan';

export const useScanStore = defineStore('scan', () => {
  const status = ref<ScanStatus>('idle');
  const scanning = ref(false);
  const filesFound = ref(0);
  const filesProcessed = ref(0);
  const currentFile = ref<string | null>(null);
  const indexingCount = ref(0);
  const unindexedCount = ref(0);
  const indexingEta = ref<number | null>(null);
  const stoppedMessage = ref(false);
  let stopTimer: ReturnType<typeof setTimeout> | null = null;

  // Scan experience state
  const phase = ref<ScanPhase>('idle');
  const dismissed = ref(false);
  const currentDirectory = ref<string | null>(null);
  const folderCount = ref(0);
  const currentFolderIndex = ref(0);
  const isPaused = ref(false);

  // Live analysis feed (the "one level deeper" view)
  const currentAnalysis = ref<AnalysisActivity | null>(null);
  const recentAnalyses = ref<{ id: string; location: string; name: string }[]>([]);
  const modelStates = ref<Record<string, ModelState>>({});
  const jobCompleted = ref(0);
  const jobTotal = ref(0);
  const throughputSamples = ref<{ t: number; c: number }[]>([]);

  // Scan log lines shown in the discovery UI
  const logLines = ref<string[]>([]);
  const MAX_LOG_LINES = 50;
  const MAX_RECENT = 8;

  const isActive = computed(() => scanning.value || status.value === 'indexing');
  const showFullScreen = computed(() => {
    if (dismissed.value || phase.value === 'idle') return false;
    // Keep overlay visible during active scan OR while showing completion screen
    return isActive.value || phase.value === 'complete';
  });
  const showCollapsedBanner = computed(() => isActive.value && dismissed.value);
  const progress = computed(() => {
    if (filesFound.value === 0) return 0;
    return Math.round((filesProcessed.value / filesFound.value) * 100);
  });

  const analyzeProgress = computed(() => {
    if (jobTotal.value > 0) return Math.round((jobCompleted.value / jobTotal.value) * 100);
    return null;
  });

  const throughputPerMin = computed(() => {
    const samples = throughputSamples.value;
    if (samples.length < 2) return null;
    const first = samples[0];
    const last = samples[samples.length - 1];
    const dtMs = last.t - first.t;
    if (dtMs < 5000) return null;
    const perMin = ((last.c - first.c) / dtMs) * 60000;
    return Number.isFinite(perMin) && perMin >= 0 ? Math.round(perMin) : null;
  });

  function fileNameOf(location: string, id: string): string {
    const parts = String(location).split(/[/\\]/);
    return parts[parts.length - 1] || id;
  }

  function pushThroughputSample(): void {
    throughputSamples.value.push({ t: Date.now(), c: jobCompleted.value });
    if (throughputSamples.value.length > 30) throughputSamples.value.shift();
  }

  async function loadIndexingStatus(): Promise<void> {
    try {
      const count = await getIndexingStatus();
      indexingCount.value = normalizeIndexingCount(count);
      if (count > 0) {
        status.value = 'indexing';
        scanning.value = false;
      }
    } catch (error) {
      console.error('[ScanStore] Failed to load indexing status:', error);
    }
  }

  async function loadUnindexedCount(): Promise<void> {
    try {
      const count = await getUnindexedCount();
      unindexedCount.value = normalizeIndexingCount(count);
    } catch (error) {
      console.error('[ScanStore] Failed to load unindexed count:', error);
    }
  }

  void listenEvent('scan-log', (payload: string) => {
    logLines.value.push(payload);
    if (logLines.value.length > MAX_LOG_LINES) {
      logLines.value = logLines.value.slice(-MAX_LOG_LINES);
    }
  });

  void listenEvent('scan-progress', (payload) => {
    if (typeof payload.files_found === 'number') filesFound.value = payload.files_found;
    if (typeof payload.files_processed === 'number') filesProcessed.value = payload.files_processed;
    currentFile.value = payload.current_file ?? null;

    if (payload.status === 'paused') {
      isPaused.value = true;
      phase.value = 'paused';
      return;
    }

    if (isPaused.value && payload.status === 'discovering') {
      isPaused.value = false;
    }

    if (payload.status === 'indexing') {
      status.value = 'indexing';
      scanning.value = false;
      // Discovery is done; stay on "Process" until the first live analysis
      // event proves the AI pipeline is actually running.
      phase.value = 'processing';
      return;
    }
    if (payload.status === 'complete') {
      status.value = 'completed';
      scanning.value = false;
      phase.value = 'complete';
      isPaused.value = false;
      void loadIndexingStatus();
      void loadUnindexedCount();
      return;
    }
    const wasActive = scanning.value || status.value === 'indexing';
    status.value = 'scanning';
    scanning.value = true;
    phase.value = 'discovering';
    // Reset dismissed only when a NEW scan starts so the guided experience shows
    // again. Periodic discovery progress events must not override minimize.
    if (!wasActive) dismissed.value = false;
    if (typeof payload.current === 'number') currentFolderIndex.value = payload.current;
    if (typeof payload.total === 'number') folderCount.value = payload.total;
    if (typeof payload.current_directory === 'string')
      currentDirectory.value = payload.current_directory;
  });

  void listenEvent('indexing-progress', (payload) => {
    status.value = 'indexing';
    scanning.value = false;
    if (phase.value === 'processing' || phase.value === 'discovering') phase.value = 'indexing';
    indexingCount.value = normalizeIndexingCount(payload.remaining);
  });

  void listenEvent('analysis-activity', (payload) => {
    status.value = 'indexing';
    scanning.value = false;
    if (phase.value === 'processing' || phase.value === 'discovering') phase.value = 'indexing';
    if (!payload?.id) return;
    currentAnalysis.value = payload;
    if (!recentAnalyses.value.some((r) => r.id === payload.id)) {
      recentAnalyses.value.unshift({
        id: payload.id,
        location: payload.location,
        name: fileNameOf(payload.location, payload.id),
      });
      if (recentAnalyses.value.length > MAX_RECENT) recentAnalyses.value.pop();
    }
    pushThroughputSample();
  });

  void listenEvent('model-progress', (payload) => {
    const model = payload.model as string | undefined;
    if (!model) return;
    modelStates.value = {
      ...modelStates.value,
      [model]: {
        pending: Number(payload.pending) || 0,
        total: Number(payload.total) || 0,
        status: String(payload.status ?? ''),
        message: typeof payload.message === 'string' ? payload.message : undefined,
      },
    };
    if (payload.status === 'running') {
      if (phase.value === 'processing' || phase.value === 'discovering') phase.value = 'indexing';
    }
  });

  void listenEvent('indexing-eta', (payload) => {
    indexingEta.value = payload.eta;
  });

  void listenEvent('indexing-job', (payload) => {
    if (payload.status === 'running') {
      status.value = 'indexing';
      scanning.value = false;
      if (phase.value === 'processing' || phase.value === 'discovering') phase.value = 'indexing';
      if (typeof payload.completed === 'number' && typeof payload.total === 'number') {
        jobCompleted.value = payload.completed;
        jobTotal.value = payload.total;
        indexingCount.value = Math.max(payload.total - payload.completed, 0);
      }
      return;
    }
    status.value = 'completed';
    scanning.value = false;
    phase.value = 'complete';
    indexingEta.value = null;
    isPaused.value = false;
    void loadUnindexedCount();
  });

  function dismiss(): void {
    dismissed.value = true;
  }

  function show(): void {
    dismissed.value = false;
  }

  async function pause(): Promise<void> {
    try {
      await pauseIndexing();
      isPaused.value = true;
    } catch (error) {
      console.error('[ScanStore] Failed to pause indexing:', error);
    }
  }

  async function resume(): Promise<void> {
    try {
      await resumeIndexing();
      isPaused.value = false;
    } catch (error) {
      console.error('[ScanStore] Failed to resume indexing:', error);
    }
  }

  async function stop(): Promise<void> {
    try {
      await abortIndexing();
    } catch (error) {
      console.error('[ScanStore] Failed to abort indexing:', error);
    }
    status.value = 'completed';
    scanning.value = false;
    phase.value = 'idle';
    indexingCount.value = 0;
    indexingEta.value = null;
    isPaused.value = false;
    stoppedMessage.value = true;
    if (stopTimer) clearTimeout(stopTimer);
    stopTimer = setTimeout(() => {
      stoppedMessage.value = false;
    }, 5000);
    void loadUnindexedCount();
  }

  function resetScanState(): void {
    phase.value = 'idle';
    dismissed.value = false;
    currentDirectory.value = null;
    folderCount.value = 0;
    currentFolderIndex.value = 0;
    filesFound.value = 0;
    filesProcessed.value = 0;
    currentFile.value = null;
    isPaused.value = false;
    logLines.value = [];
    currentAnalysis.value = null;
    recentAnalyses.value = [];
    modelStates.value = {};
    jobCompleted.value = 0;
    jobTotal.value = 0;
    throughputSamples.value = [];
  }

  return {
    status,
    scanning,
    filesFound,
    filesProcessed,
    currentFile,
    indexingCount,
    unindexedCount,
    indexingEta,
    stoppedMessage,
    phase,
    dismissed,
    currentDirectory,
    folderCount,
    currentFolderIndex,
    isPaused,
    isActive,
    showFullScreen,
    showCollapsedBanner,
    progress,
    logLines,
    currentAnalysis,
    recentAnalyses,
    modelStates,
    jobCompleted,
    jobTotal,
    analyzeProgress,
    throughputPerMin,
    loadIndexingStatus,
    loadUnindexedCount,
    dismiss,
    show,
    pause,
    resume,
    stop,
    resetScanState,
  };
});
