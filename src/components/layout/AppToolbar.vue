<script setup lang="ts">
import SearchBar from '@/components/search/SearchBar.vue';
import { useSearchStore } from '@/stores/search';
import { useAppStore } from '@/stores/app';
import { useScanStore } from '@/stores/scan';
import { useI18n } from 'vue-i18n';

const { t } = useI18n();
const searchStore = useSearchStore();
const appStore = useAppStore();
const scanStore = useScanStore();

const sortOptions = [
  { value: 'newest', label: () => t('search.sort_newest'), icon: 'mdi-sort-calendar-descending' },
  { value: 'best', label: () => t('search.sort_best'), icon: 'mdi-star' },
  { value: 'random', label: () => t('search.sort_random'), icon: 'mdi-dice-multiple' },
] as const;

function handleScanClick(): void {
  scanStore.show();
  if (!scanStore.isActive) {
    appStore.startScan();
  }
}
</script>

<template>
  <v-app-bar elevation="0" color="surface" class="px-2">
    <div class="toolbar-row">
      <div class="toolbar-search">
        <SearchBar />
      </div>
      <div class="toolbar-actions">
        <div class="d-flex ga-1 align-center toolbar-btns">
          <v-tooltip location="top">
            <template #activator="{ props: tipProps }">
              <v-btn
                v-bind="tipProps"
                icon
                size="small"
                :variant="scanStore.isActive ? 'flat' : 'text'"
                :color="scanStore.isActive ? 'primary' : undefined"
                :aria-label="scanStore.isActive ? t('scan.view_progress') : t('scan.scan_button')"
                @click="handleScanClick"
              >
                <v-progress-circular
                  v-if="scanStore.isActive"
                  indeterminate
                  size="16"
                  width="2"
                  color="rgb(var(--v-theme-on-primary))"
                />
                <v-icon v-else size="16">mdi-magnify-scan</v-icon>
              </v-btn>
            </template>
            <span>{{ scanStore.isActive ? t('scan.view_progress') : t('scan.scan_button') }}</span>
          </v-tooltip>
          <v-tooltip v-for="opt in sortOptions" :key="opt.value" location="top">
            <template #activator="{ props: tipProps }">
              <v-btn
                v-bind="tipProps"
                icon
                size="small"
                :variant="searchStore.sortOrder === opt.value ? 'flat' : 'text'"
                :color="searchStore.sortOrder === opt.value ? 'primary' : undefined"
                :aria-label="opt.label()"
                @click="searchStore.setSortOrder(opt.value)"
              >
                <v-icon size="16">{{ opt.icon }}</v-icon>
              </v-btn>
            </template>
            <span>{{ opt.label() }}</span>
          </v-tooltip>
        </div>
      </div>
    </div>
  </v-app-bar>
</template>

<style scoped>
.toolbar-row {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 16px;
  width: 100%;
  padding: 0 8px;
}

.toolbar-search {
  flex: 1 1 auto;
  min-width: 0;
}

.toolbar-actions {
  flex: 0 0 auto;
}

@media (max-width: 640px) {
  .toolbar-row {
    gap: 10px;
  }
}

@media (max-width: 440px) {
  .toolbar-row {
    gap: 6px;
    padding: 0 4px;
  }

  .toolbar-actions .toolbar-btns {
    gap: 2px;
  }

  .toolbar-actions :deep(.v-btn) {
    width: 36px;
    height: 36px;
  }
}
</style>
