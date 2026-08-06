<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { dayCounts } from '@/services/tauri';
import type { DayCount } from '@/types/search';

const props = defineProps<{
  modelValue: [string, string] | null;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: [string, string] | null];
}>();

const { t } = useI18n();

const viewYear = ref<number>(new Date().getFullYear());
const viewMonth = ref<number>(new Date().getMonth());
const counts = ref<DayCount[]>([]);
let fetchTimer: ReturnType<typeof setTimeout> | null = null;

const locale = localStorage.getItem('siegu_language') || 'en';

const weekdayLabels = computed(() => {
  const base = new Date(2024, 0, 1);
  const labels: string[] = [];
  for (let i = 1; i <= 7; i++) {
    const d = new Date(base);
    d.setDate(base.getDate() + i);
    labels.push(d.toLocaleDateString(locale, { weekday: 'short' }));
  }
  return labels;
});

const monthLabel = computed(() => {
  const d = new Date(viewYear.value, viewMonth.value, 1);
  const label = d.toLocaleDateString(locale, { month: 'long', year: 'numeric' });
  return label.charAt(0).toUpperCase() + label.slice(1);
});

const countsByDate = computed<Record<string, { photos: number; videos: number }>>(() => {
  const map: Record<string, { photos: number; videos: number }> = {};
  for (const c of counts.value) {
    map[c.date] = { photos: c.photos, videos: c.videos };
  }
  return map;
});

const cells = computed<
  {
    key: string;
    date: string | null;
    day: number;
    count: { photos: number; videos: number } | null;
  }[]
>(() => {
  const first = new Date(viewYear.value, viewMonth.value, 1);
  const daysInMonth = new Date(viewYear.value, viewMonth.value + 1, 0).getDate();
  const offset = (first.getDay() + 6) % 7;
  const result: {
    key: string;
    date: string | null;
    day: number;
    count: { photos: number; videos: number } | null;
  }[] = [];
  for (let i = 0; i < offset; i++) {
    result.push({ key: `pad-${i}`, date: null, day: 0, count: null });
  }
  for (let d = 1; d <= daysInMonth; d++) {
    const dateStr = `${viewYear.value}-${String(viewMonth.value + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
    result.push({
      key: dateStr,
      date: dateStr,
      day: d,
      count: countsByDate.value[dateStr] ?? null,
    });
  }
  return result;
});

const displayRange = ref<[string, string] | null>(null);
let pendingStart: string | null = null;

watch(
  () => props.modelValue,
  (value) => {
    displayRange.value = value;
    if (!value) pendingStart = null;
  },
  { immediate: true },
);

function isStart(date: string): boolean {
  return displayRange.value !== null && displayRange.value[0] === date;
}

function isEnd(date: string): boolean {
  return displayRange.value !== null && displayRange.value[1] === date;
}

function isInRange(date: string): boolean {
  if (!displayRange.value) return false;
  const [a, b] = displayRange.value;
  return date > a && date < b;
}

function selectDate(date: string): void {
  if (pendingStart === null) {
    pendingStart = date;
    displayRange.value = [date, date];
    emit('update:modelValue', [date, date]);
    return;
  }
  const from = pendingStart < date ? pendingStart : date;
  const to = pendingStart < date ? date : pendingStart;
  pendingStart = null;
  const complete: [string, string] = [from, to];
  displayRange.value = complete;
  emit('update:modelValue', complete);
}

function clear(): void {
  pendingStart = null;
  displayRange.value = null;
  emit('update:modelValue', null);
}

function prevMonth(): void {
  if (viewMonth.value === 0) {
    viewMonth.value = 11;
    viewYear.value -= 1;
  } else {
    viewMonth.value -= 1;
  }
}

function nextMonth(): void {
  if (viewMonth.value === 11) {
    viewMonth.value = 0;
    viewYear.value += 1;
  } else {
    viewMonth.value += 1;
  }
}

function todayStr(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
}

function goToday(): void {
  const now = new Date();
  viewYear.value = now.getFullYear();
  viewMonth.value = now.getMonth();
}

async function fetchCounts(): Promise<void> {
  const first = `${viewYear.value}-${String(viewMonth.value + 1).padStart(2, '0')}-01`;
  const last = `${viewYear.value}-${String(viewMonth.value + 1).padStart(2, '0')}-31`;
  try {
    counts.value = await dayCounts(first, last);
  } catch (error) {
    console.error('[DateRangePicker] Failed to load day counts:', error);
    counts.value = [];
  }
}

watch([viewYear, viewMonth], () => {
  if (fetchTimer) clearTimeout(fetchTimer);
  fetchTimer = setTimeout(() => {
    void fetchCounts();
  }, 120);
});

onMounted(() => {
  void fetchCounts();
});

onUnmounted(() => {
  if (fetchTimer) clearTimeout(fetchTimer);
  fetchTimer = null;
});
</script>

<template>
  <div class="date-range-picker">
    <div class="drp-header">
      <div class="drp-range-summary">
        <template v-if="displayRange">
          <v-icon size="16" class="mr-1">mdi-calendar-range</v-icon>
          <span class="drp-range-text">{{ displayRange[0] }}</span>
          <v-icon size="14" class="mx-1">mdi-arrow-right</v-icon>
          <span class="drp-range-text">{{ displayRange[1] }}</span>
        </template>
        <template v-else>
          <span class="drp-range-text text-muted">{{ t('search.date_picker.hint') }}</span>
        </template>
      </div>
      <button v-if="displayRange" class="drp-clear" @click="clear">
        {{ t('search.date_picker.clear') }}
      </button>
    </div>

    <div class="drp-nav">
      <button class="drp-nav-btn" @click="prevMonth">
        <v-icon size="18">mdi-chevron-left</v-icon>
      </button>
      <span class="drp-month-label">{{ monthLabel }}</span>
      <button class="drp-nav-btn" @click="nextMonth">
        <v-icon size="18">mdi-chevron-right</v-icon>
      </button>
    </div>

    <div class="drp-weekdays">
      <span v-for="label in weekdayLabels" :key="label" class="drp-weekday">{{ label }}</span>
    </div>

    <div class="drp-grid">
      <button
        v-for="cell in cells"
        :key="cell.key"
        class="drp-cell"
        :class="{
          'drp-cell--start': cell.date && isStart(cell.date),
          'drp-cell--end': cell.date && isEnd(cell.date),
          'drp-cell--inrange': cell.date && isInRange(cell.date),
          'drp-cell--today': cell.date === todayStr(),
        }"
        :disabled="!cell.date"
        @click="cell.date && selectDate(cell.date)"
      >
        <template v-if="cell.date">
          <span class="drp-day">{{ cell.day }}</span>
          <span v-if="cell.count" class="drp-counts">
            <span class="drp-count drp-count--photos">{{ cell.count.photos }}</span>
            <span v-if="cell.count.videos" class="drp-count drp-count--videos">{{
              cell.count.videos
            }}</span>
          </span>
        </template>
      </button>
    </div>

    <div class="drp-footer">
      <button class="drp-today" @click="goToday">
        <v-icon size="14" class="mr-1">mdi-calendar-today</v-icon>
        {{ t('search.date_picker.today') }}
      </button>
      <div class="drp-legend">
        <span class="drp-legend-dot drp-legend-dot--photos"></span>
        <span class="drp-legend-text">{{ t('search.date_picker.photos') }}</span>
        <span class="drp-legend-dot drp-legend-dot--videos"></span>
        <span class="drp-legend-text">{{ t('search.date_picker.videos') }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.date-range-picker {
  background: var(--color-bg-hover);
  border-radius: 16px;
  padding: 12px;
}

.date-range-picker button {
  border: none;
  -webkit-appearance: none;
  appearance: none;
}

.drp-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.drp-range-summary {
  display: flex;
  align-items: center;
  font-size: 12px;
  font-weight: 700;
  color: var(--color-text-primary);
  overflow: hidden;
}

.drp-range-text {
  white-space: nowrap;
}

.drp-clear {
  font-size: 11px;
  font-weight: 700;
  color: var(--color-text-muted);
  background: transparent;
  padding: 3px 8px;
  border-radius: 999px;
  cursor: pointer;
  user-select: none;
}

.drp-clear:hover {
  color: var(--color-text-primary);
  background: color-mix(in srgb, var(--color-text-primary) 8%, transparent);
}

.drp-nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}

.drp-nav-btn {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  user-select: none;
}

.drp-nav-btn:hover {
  background: color-mix(in srgb, var(--color-text-primary) 8%, transparent);
  color: var(--color-text-primary);
}

.drp-month-label {
  font-size: 13px;
  font-weight: 800;
  color: var(--color-text-primary);
}

.drp-weekdays {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  margin-bottom: 4px;
}

.drp-weekday {
  text-align: center;
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  color: var(--color-text-muted);
  padding: 2px 0;
}

.drp-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 2px;
}

.drp-cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
  height: 38px;
  border-radius: 10px;
  background: transparent;
  cursor: pointer;
  user-select: none;
  position: relative;
  transition: background 0.12s ease;
}

.drp-cell:not(:disabled):hover {
  background: color-mix(in srgb, var(--color-text-primary) 10%, transparent);
}

.drp-day {
  font-size: 12px;
  font-weight: 600;
  line-height: 1.1;
  color: var(--color-text-primary);
}

.drp-counts {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  line-height: 1;
}

.drp-count {
  font-size: 9px;
  font-weight: 800;
  padding: 0 3px;
  border-radius: 4px;
  color: #fff;
}

.drp-count--photos {
  background: color-mix(in srgb, var(--color-brand-faces) 85%, transparent);
}

.drp-count--videos {
  background: color-mix(in srgb, var(--color-brand-videos) 85%, transparent);
}

.drp-cell--inrange {
  background: color-mix(in srgb, var(--color-bg-btn) 18%, transparent);
  border-radius: 0;
}

.drp-cell--start,
.drp-cell--end {
  background: var(--color-bg-btn);
  border-radius: 10px;
}

.drp-cell--start .drp-day,
.drp-cell--end .drp-day {
  color: #fff;
  font-weight: 800;
}

.drp-cell--today .drp-day::after {
  content: '';
  display: block;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--color-text-primary);
  margin: 1px auto 0;
}

.drp-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 8px;
  padding-top: 8px;
}

.drp-today {
  display: inline-flex;
  align-items: center;
  font-size: 11px;
  font-weight: 700;
  color: var(--color-text-secondary);
  background: transparent;
  cursor: pointer;
  user-select: none;
  padding: 3px 8px;
  border-radius: 999px;
}

.drp-today:hover {
  color: var(--color-text-primary);
  background: color-mix(in srgb, var(--color-text-primary) 8%, transparent);
}

.drp-legend {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.drp-legend-dot {
  width: 8px;
  height: 8px;
  border-radius: 3px;
}

.drp-legend-dot--photos {
  background: color-mix(in srgb, var(--color-brand-faces) 85%, transparent);
}

.drp-legend-dot--videos {
  background: color-mix(in srgb, var(--color-brand-videos) 85%, transparent);
}

.drp-legend-text {
  font-size: 10px;
  font-weight: 600;
  color: var(--color-text-muted);
  margin-right: 4px;
}

.text-muted {
  color: var(--color-text-muted);
}
</style>
