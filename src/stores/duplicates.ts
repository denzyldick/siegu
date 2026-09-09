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
  // Three-step stepper state. `stageDone[a]` is true once that stage finished;
  // the active stage name comes from the latest progress event.
  const activeStage = ref<string>('');
  const stageDone = ref<Record<string, boolean>>({});

  let listenersRegistered = false;

  function registerDesktopListeners(): void {
    if (listenersRegistered) return;
    listenersRegistered = true;

    void listen<ScanProgress>('duplicate-scan-progress', (e) => {
      const p = e.payload;
      progress.value = p;
      scanning.value = true;
      if (p.stage) {
        // A new stage name marks the previous one as finished (the backend only
        // fires per-photo progress while there is actual work, so a zero-work
        // stage like hashing emits just a single marker before moving on).
        if (p.stage !== activeStage.value && activeStage.value) {
          stageDone.value[activeStage.value] = true;
        }
        activeStage.value = p.stage;
        if (p.total > 0 && p.done >= p.total) stageDone.value[p.stage] = true;
      }
    });
    void listen<DuplicateScanResult>('duplicate-scan-done', (e) => {
      applyResult(e.payload);
      scanning.value = false;
      progress.value = { done: 0, total: 0 };
      // Mark any remaining stages as done when the scan wraps up.
      if (activeStage.value) stageDone.value[activeStage.value] = true;
      activeStage.value = '';
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
    activeStage.value = '';
    stageDone.value = {};
    groups.value = [];
    stats.value = null;
    libraryBytes.value = 0;
    photoCount.value = 0;
    videoCount.value = 0;
    ready.value = false;
  }

  async function includeVideos(): Promise<boolean> {
    try {
      const configStr = await invoke<string>('get_config');
      const config = JSON.parse(configStr) as Record<string, string>;
      return config.dup_scan_videos === 'true';
    } catch {
      return false;
    }
  }

  async function startScan(force = false): Promise<void> {
    if (isTauriRuntime) {
      if (!listenersRegistered) registerDesktopListeners();
      scanning.value = true;
      progress.value = { done: 0, total: 0 };
      const include_videos = await includeVideos();
      await invoke('start_duplicate_scan', {
        include_clip: true,
        include_videos,
      }).catch(() => {
        scanning.value = false;
      });
    } else {
      // Guest/web host computes synchronously; derive stats client-side.
      if (!force && ready.value) return;
      scanning.value = true;
      try {
        const found = await invoke<DuplicateGroupView[]>('find_duplicates', {
          include_clip: true,
          include_videos: await includeVideos(),
        });
        applyResult({
          groups: found,
          stats: {
            group_count: found.length,
            duplicate_count: found.reduce((sum, g) => sum + Math.max(0, g.members.length - 1), 0),
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

  // Move an arbitrary set of member ids to the trash (multiple groups, selected
  // checkboxes, or the whole group minus the kept one all funnel through here).
  async function trashMembers(ids: string[]): Promise<number> {
    const unique = [...new Set(ids)];
    if (unique.length === 0) return 0;
    const removed = await invoke<number>('trash_duplicate_members', { ids: unique });
    if (removed <= 0) return 0;

    const trashed = new Set(unique);
    const next: DuplicateGroupView[] = [];
    const st = stats.value;
    for (const group of groups.value) {
      const kept = group.members.filter((m) => !trashed.has(m.id));
      // A group left with fewer than two members is resolved: it is no longer
      // a duplicate, so drop it entirely.
      if (kept.length < 2) continue;
      // Keep the best-scored member as the group's default when the previous
      // best_id got trashed.
      const bestStillKept = group.best_id != null && trashed.has(group.best_id);
      if (bestStillKept) {
        const first = kept[0];
        next.push({ ...group, members: kept, best_id: first?.id ?? null });
      } else {
        next.push({ ...group, members: kept });
      }
    }
    groups.value = next;
    if (st) {
      st.group_count = next.length;
      st.duplicate_count = Math.max(
        0,
        next.reduce((sum, g) => sum + Math.max(0, g.members.length - 1), 0),
      );
      // Sum the remaining groups' reclaimable bytes; untouched groups keep their
      // exact value, emptied groups drop theirs.
      st.reclaimable_bytes = next.reduce((sum, g) => sum + g.reclaimable_bytes, 0);
    }
    return removed;
  }

  return {
    scanning,
    progress,
    activeStage,
    stageDone,
    groups,
    stats,
    libraryBytes,
    photoCount,
    videoCount,
    ready,
    startScan,
    ensureLoaded,
    trashGroup,
    trashMembers,
    resetRuntimeState,
  };
});
