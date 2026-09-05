import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke, isTauriRuntime, listen } from '@/services/invoke';
import { useGlobalSnackbar } from '@/composables/useGlobalSnackbar';
import { formatBytes } from '@/utils/format';
import type {
  DuplicateGroupView,
  DuplicateScanResult,
  DuplicateStats,
  ScanProgress,
} from '@/types/duplicates';

// The scan lives here (not in the page component) so it keeps running and can
// notify the user even after they navigate away from the Space Saver page.
export const useDuplicatesStore = defineStore('duplicates', () => {
  const { t } = useI18n();
  const snackbar = useGlobalSnackbar();

  const scanning = ref(false);
  const progress = ref<ScanProgress>({ done: 0, total: 0 });
  const groups = ref<DuplicateGroupView[]>([]);
  const stats = ref<DuplicateStats | null>(null);
  const libraryBytes = ref(0);
  const photoCount = ref(0);
  const videoCount = ref(0);
  const ready = ref(false);

  let listenersRegistered = false;

  function registerDesktopListeners(): void {
    if (listenersRegistered) return;
    listenersRegistered = true;

    void listen<ScanProgress>('duplicate-scan-progress', (e) => {
      progress.value = e.payload;
      scanning.value = true;
    });
    void listen<DuplicateScanResult>('duplicate-scan-done', (e) => {
      applyResult(e.payload);
      scanning.value = false;
      progress.value = { done: 0, total: 0 };
      const st = stats.value;
      if (st && st.group_count > 0) {
        snackbar.show(
          t('duplicates.scan_done', {
            groups: st.group_count.toLocaleString(),
            size: formatBytes(st.reclaimable_bytes),
          }),
          'success',
        );
      } else {
        snackbar.show(t('duplicates.scan_clean'), 'success');
      }
    });
  }

  function applyResult(result: DuplicateScanResult): void {
    groups.value = result.groups;
    stats.value = result.stats;
    libraryBytes.value = result.library_bytes ?? 0;
    photoCount.value = result.photo_count ?? 0;
    videoCount.value = result.video_count ?? 0;
    ready.value = true;
  }

  function resetRuntimeState(): void {
    scanning.value = false;
    progress.value = { done: 0, total: 0 };
    groups.value = [];
    stats.value = null;
    libraryBytes.value = 0;
    photoCount.value = 0;
    videoCount.value = 0;
    ready.value = false;
  }

  async function startScan(force = false): Promise<void> {
    if (isTauriRuntime) {
      if (!listenersRegistered) registerDesktopListeners();
      scanning.value = true;
      progress.value = { done: 0, total: 0 };
      await invoke('start_duplicate_scan', { include_clip: false }).catch(() => {
        scanning.value = false;
      });
    } else if (!force && ready.value) {
      // Guest/web host computes synchronously; derive stats client-side.
      if (!force && ready.value) return;
      scanning.value = true;
      try {
        const found = await invoke<DuplicateGroupView[]>('find_duplicates', {
          include_clip: false,
        });
        applyResult({
          groups: found,
          stats: {
            group_count: found.length,
            duplicate_count: found.reduce(
              (sum, g) => sum + Math.max(0, g.members.length - 1),
              0,
            ),
            reclaimable_bytes: found.reduce((sum, g) => sum + g.reclaimable_bytes, 0),
          },
          library_bytes: 0,
          photo_count: 0,
          video_count: 0,
        });
      } finally {
        scanning.value = false;
      }
    }
  }

  // If we already have results (e.g. the scan completed while on another page)
  // just show them; otherwise kick off a fresh scan.
  async function ensureLoaded(): Promise<void> {
    if (isTauriRuntime) {
      if (!scanning.value && !ready.value) await startScan();
    } else {
      await startScan();
    }
  }

  async function trashGroup(gi: number, keep: string): Promise<number> {
    const group = groups.value[gi];
    if (!group) return 0;
    const ids = group.members.filter((m) => m.id !== keep).map((m) => m.id);
    if (ids.length === 0) return 0;
    const removed = await invoke<number>('trash_duplicate_members', { ids });
    const st = stats.value;
    if (removed > 0 && st) {
      st.group_count = Math.max(0, st.group_count - 1);
      st.duplicate_count = Math.max(0, st.duplicate_count - removed);
      st.reclaimable_bytes = Math.max(0, st.reclaimable_bytes - group.reclaimable_bytes);
    }
    groups.value.splice(gi, 1);
    return removed;
  }

  return {
    scanning,
    progress,
    groups,
    stats,
    libraryBytes,
    photoCount,
    videoCount,
    ready,
    startScan,
    ensureLoaded,
    trashGroup,
    resetRuntimeState,
  };
});