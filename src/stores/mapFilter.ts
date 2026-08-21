import { defineStore } from 'pinia';
import { ref } from 'vue';

export const useMapFilterStore = defineStore('mapFilter', () => {
  const dateFrom = ref<string | null>(null);
  const dateTo = ref<string | null>(null);

  function setDateRange(from: string | null, to: string | null): void {
    dateFrom.value = from;
    dateTo.value = to;
  }

  function clearDateRange(): void {
    dateFrom.value = null;
    dateTo.value = null;
  }

  return { dateFrom, dateTo, setDateRange, clearDateRange };
});
