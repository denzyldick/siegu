import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { getIndexingStatus, getUnindexedCount, abortIndexing } from '@/services/tauri';
import { listenEvent } from '@/services/events';
import { normalizeIndexingCount } from '@/composables/useMediaUtils';
import type { ScanStatus } from '@/types/scan';

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

  const isActive = computed(() => scanning.value || status.value === 'indexing');
  const progress = computed(() => {
    if (filesFound.value === 0) return 0;
    return Math.round((filesProcessed.value / filesFound.value) * 100);
  });

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

  void listenEvent('scan-progress', (payload) => {
    if (typeof payload.files_found === 'number') filesFound.value = payload.files_found;
    if (typeof payload.files_processed === 'number') filesProcessed.value = payload.files_processed;
    currentFile.value = payload.current_file ?? null;

    if (payload.status === 'indexing') {
      status.value = 'indexing';
      scanning.value = false;
      return;
    }
    if (payload.status === 'complete') {
      status.value = 'completed';
      scanning.value = false;
      void loadIndexingStatus();
      void loadUnindexedCount();
      return;
    }
    status.value = 'scanning';
    scanning.value = true;
  });

  void listenEvent('indexing-progress', (payload) => {
    status.value = 'indexing';
    scanning.value = false;
    indexingCount.value = normalizeIndexingCount(payload.remaining);
  });

  void listenEvent('indexing-eta', (payload) => {
    indexingEta.value = payload.eta;
  });

  void listenEvent('indexing-job', (payload) => {
    if (payload.status === 'running') {
      status.value = 'indexing';
      scanning.value = false;
      if (typeof payload.completed === 'number' && typeof payload.total === 'number') {
        indexingCount.value = Math.max(payload.total - payload.completed, 0);
      }
      return;
    }
    status.value = 'completed';
    scanning.value = false;
    indexingEta.value = null;
    void loadUnindexedCount();
  });

  async function stop(): Promise<void> {
    try {
      await abortIndexing();
    } catch (error) {
      console.error('[ScanStore] Failed to abort indexing:', error);
    }
    status.value = 'completed';
    scanning.value = false;
    indexingCount.value = 0;
    indexingEta.value = null;
    stoppedMessage.value = true;
    if (stopTimer) clearTimeout(stopTimer);
    stopTimer = setTimeout(() => {
      stoppedMessage.value = false;
    }, 5000);
    void loadUnindexedCount();
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
    isActive,
    progress,
    loadIndexingStatus,
    loadUnindexedCount,
    stop,
  };
});
