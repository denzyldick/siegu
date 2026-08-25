import { computed, ref } from 'vue';
import type { MediaItem } from '@/types/media';

export interface TimePeriod {
  label: string;
  sortKey: string;
  startIndex: number;
  endIndex: number;
}

export function useTimePeriods(photos: () => MediaItem[]) {
  const overlayLabel = ref('');
  const overlayVisible = ref(false);
  let overlayTimer: ReturnType<typeof setTimeout> | undefined;

  const periods = computed<TimePeriod[]>(() => {
    const list = photos();
    if (!list || list.length === 0) return [];

    const result: TimePeriod[] = [];
    let current: TimePeriod | null = null;

    for (let i = 0; i < list.length; i++) {
      const key = list[i]._groupKey ?? 'Recent';
      const sort = list[i]._sortKey ?? '0';

      if (!current || current.label !== key) {
        if (current) current.endIndex = i - 1;
        current = { label: key, sortKey: sort, startIndex: i, endIndex: i };
        result.push(current);
      } else {
        current.endIndex = i;
      }
    }

    return result;
  });

  function findPeriodIndex(index: number): number {
    const list = periods.value;
    for (let i = 0; i < list.length; i++) {
      if (index >= list[i].startIndex && index <= list[i].endIndex) return i;
    }
    return 0;
  }

  function showOverlay(label: string): void {
    overlayLabel.value = label;
    overlayVisible.value = true;
    if (overlayTimer) clearTimeout(overlayTimer);
    overlayTimer = setTimeout(() => {
      overlayVisible.value = false;
    }, 1200);
  }

  function jumpToPrevious(currentIndex: number): number | null {
    const pIdx = findPeriodIndex(currentIndex);
    if (pIdx <= 0) return null;
    const prev = periods.value[pIdx - 1];
    showOverlay(prev.label);
    return prev.startIndex;
  }

  function jumpToNext(currentIndex: number): number | null {
    const pIdx = findPeriodIndex(currentIndex);
    if (pIdx >= periods.value.length - 1) return null;
    const next = periods.value[pIdx + 1];
    showOverlay(next.label);
    return next.startIndex;
  }

  function currentPeriodLabel(index: number): string {
    const pIdx = findPeriodIndex(index);
    return periods.value[pIdx]?.label ?? '';
  }

  return {
    periods,
    overlayLabel,
    overlayVisible,
    jumpToPrevious,
    jumpToNext,
    currentPeriodLabel,
  };
}
