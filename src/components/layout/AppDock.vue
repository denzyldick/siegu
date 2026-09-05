<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useUiStore } from '@/stores/ui';
import { useScanStore } from '@/stores/scan';
import { useRuntimeStore } from '@/stores/runtime';
import { useDuplicatesStore } from '@/stores/duplicates';
import { normalizeIndexingCount, formatEta } from '@/composables/useMediaUtils';
import logo from '@/assets/logo.png';

const { t } = useI18n();
const uiStore = useUiStore();
const scanStore = useScanStore();
const runtimeStore = useRuntimeStore();
const dupeStore = useDuplicatesStore();

// Hide the dock while the user is scrolling the page; it reappears once
// scrolling stops (idle) or when hovered.
const scrolledAway = ref(false);
const dockHovered = ref(false);
const idleTimer = ref<ReturnType<typeof setTimeout> | null>(null);

const dockHidden = computed(() => scrolledAway.value && !dockHovered.value);

function hideWhileScrolling(): void {
  scrolledAway.value = true;
  if (idleTimer.value) clearTimeout(idleTimer.value);
  idleTimer.value = setTimeout(() => {
    scrolledAway.value = false;
  }, 350);
}

// The content area scrolls on `document` (window scrolls the page). With
// page-mode virtual scrollers the scroll event still surfaces on document, so
// a single capture listener on document is enough to catch all scrolls.
function onScroll(): void {
  hideWhileScrolling();
}

onMounted(() => {
  document.addEventListener('scroll', onScroll, { passive: true, capture: true });
});

onUnmounted(() => {
  document.removeEventListener('scroll', onScroll, { capture: true } as EventListenerOptions);
  if (idleTimer.value) clearTimeout(idleTimer.value);
});

const allNavItems = [
  {
    page: 'home' as const,
    icon: null,
    tour: 'dock-home',
    useLogo: true,
    label: 'dock.home' as const,
  },
  {
    page: 'collections' as const,
    icon: 'mdi-album',
    tour: 'dock-collections',
    useLogo: false,
    label: 'dock.collections' as const,
  },
  {
    page: 'location' as const,
    icon: 'mdi-map-outline',
    tour: 'dock-map',
    useLogo: false,
    label: 'dock.map' as const,
  },
  {
    page: 'devices' as const,
    icon: 'mdi-laptop',
    tour: 'dock-devices',
    useLogo: false,
    label: 'dock.devices' as const,
  },
  {
    page: 'duplicates' as const,
    icon: 'mdi-file-multiple-outline',
    tour: 'dock-duplicates',
    useLogo: false,
    label: 'dock.duplicates' as const,
  },
  {
    page: 'settings' as const,
    icon: 'mdi-cog-outline',
    tour: 'dock-settings',
    useLogo: false,
    label: 'dock.settings' as const,
  },
];

const GUEST_PAGES: Array<(typeof allNavItems)[number]['page']> = ['home', 'settings'];

const navItems = computed(() => {
  if (runtimeStore.isGuest) {
    return allNavItems.filter((item) => GUEST_PAGES.includes(item.page));
  }
  return allNavItems;
});

const isIndexing = computed(() => scanStore.isActive);
const jobsLeft = computed(() => normalizeIndexingCount(scanStore.indexingCount));
const dupeCount = computed(() => dupeStore.stats?.group_count ?? 0);
const statusLabel = computed(() => {
  if (!scanStore.isActive) return '';
  if (scanStore.status === 'scanning') return t('sync.scanning');
  if (scanStore.indexingCount > 0) {
    return t('sync.indexing');
  }
  return t('sync.indexing');
});

const tooltipText = computed(() => {
  if (!scanStore.isActive) return '';
  const eta =
    scanStore.indexingEta && scanStore.indexingEta > 0 ? formatEta(scanStore.indexingEta) : null;
  if (scanStore.indexingCount > 0) {
    return eta
      ? `${statusLabel.value}: ${t('sync.jobs_left', { count: jobsLeft.value.toLocaleString() })} · ~${eta}`
      : `${statusLabel.value}: ${t('sync.jobs_left', { count: jobsLeft.value.toLocaleString() })}`;
  }
  return statusLabel.value;
});

function navigate(
  page: 'home' | 'collections' | 'location' | 'devices' | 'duplicates' | 'settings',
): void {
  uiStore.setPage(page);
}
</script>

<template>
  <div class="dock-container" :class="{ 'dock-hidden': dockHidden }">
    <v-sheet
      class="dock d-flex justify-space-around align-center pa-2 rounded-pill mb-8"
      elevation="0"
      width="100%"
      max-width="380"
      color="surface"
      @mouseenter="dockHovered = true"
      @mouseleave="dockHovered = false"
    >
      <template v-for="item in navItems" :key="item.page">
        <v-tooltip location="top">
          <template v-slot:activator="{ props: tooltipProps }">
            <v-btn
              v-bind="tooltipProps"
              icon
              variant="text"
              size="small"
              class="siegu-dock-btn"
              :class="{ 'siegu-dock-btn--active': uiStore.currentPage === item.page }"
              :data-tour="item.tour"
              @click="navigate(item.page)"
            >
              <template v-if="item.useLogo">
                <div class="siegu-logo-wrap">
                  <v-img
                    :src="logo"
                    width="24"
                    height="24"
                    :class="uiStore.currentPage === item.page ? 'siegu-logo--active' : 'opacity-40'"
                  />
                  <template v-if="isIndexing">
                    <span class="indexing-dot" aria-label="indexing"></span>
                    <span v-if="jobsLeft > 0" class="indexing-pill">{{
                      jobsLeft.toLocaleString()
                    }}</span>
                  </template>
                </div>
              </template>
              <template v-else>
                <div class="siegu-dock-icon-wrap">
                  <v-icon size="24">{{ item.icon }}</v-icon>
                  <template v-if="item.page === 'duplicates'">
                    <span v-if="dupeStore.scanning" class="duplicates-dot" aria-label="scanning"></span>
                    <span v-if="dupeStore.ready && dupeCount > 0" class="duplicates-pill">{{
                      dupeCount.toLocaleString()
                    }}</span>
                  </template>
                </div>
              </template>
            </v-btn>
          </template>
          <span v-if="item.useLogo && isIndexing">{{ tooltipText }}</span>
          <span v-else-if="item.page === 'duplicates' && dupeStore.scanning">{{
            t('duplicates.scanning')
          }}</span>
          <span v-else>{{ t(item.label) }}</span>
        </v-tooltip>
      </template>
    </v-sheet>
  </div>
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
  opacity: 1;
  transform: translateY(0);
  transition:
    opacity 0.28s cubic-bezier(0.4, 0, 0.2, 1),
    transform 0.28s cubic-bezier(0.4, 0, 0.2, 1);
}

.dock-container.dock-hidden {
  opacity: 0;
  transform: translateY(130%);
  pointer-events: none;
}

.dock {
  pointer-events: auto;
  backdrop-filter: blur(16px);
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
}

.siegu-dock-btn {
  color: rgba(var(--v-theme-on-surface), 0.6) !important;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1) !important;
  border-radius: 50% !important;
}

.siegu-dock-btn:hover {
  background: color-mix(in srgb, rgb(var(--v-theme-on-surface)) 6%, transparent) !important;
  color: rgb(var(--v-theme-on-surface)) !important;
  transform: translateY(-2px);
}

.siegu-dock-btn--active {
  color: rgb(var(--v-theme-on-primary)) !important;
  background: rgb(var(--v-theme-primary)) !important;
}

.siegu-dock-btn--active:hover {
  background: rgb(var(--v-theme-primary)) !important;
  filter: brightness(0.85);
}

.siegu-logo--active {
  filter: invert(1) !important;
}

[data-theme='dark'] .siegu-logo-wrap .v-img:not(.siegu-logo--active) {
  filter: invert(1);
}

.siegu-logo-wrap {
  position: relative;
  display: inline-flex;
}

.indexing-dot {
  position: absolute;
  top: -3px;
  right: -3px;
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: rgb(var(--v-theme-error));
  animation: siegu-pulse 1.6s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

.indexing-pill {
  position: absolute;
  bottom: -5px;
  right: -10px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 9999px;
  background: rgb(var(--v-theme-error));
  color: #fff;
  font-size: 9px;
  font-weight: 800;
  line-height: 16px;
  text-align: center;
  border: 2px solid rgb(var(--v-theme-surface));
}

.siegu-dock-icon-wrap {
  position: relative;
  display: inline-flex;
}

.duplicates-dot {
  position: absolute;
  top: -3px;
  right: -3px;
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: rgb(var(--v-theme-primary));
  animation: dupe-pulse 1.6s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

.duplicates-pill {
  position: absolute;
  bottom: -5px;
  right: -10px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 9999px;
  background: rgb(var(--v-theme-primary));
  color: #fff;
  font-size: 9px;
  font-weight: 800;
  line-height: 16px;
  text-align: center;
  border: 2px solid rgb(var(--v-theme-surface));
}

@keyframes dupe-pulse {
  0% {
    box-shadow: 0 0 0 0 color-mix(in srgb, rgb(var(--v-theme-primary)) 60%, transparent);
  }
  70% {
    box-shadow: 0 0 0 7px transparent;
  }
  100% {
    box-shadow: 0 0 0 0 transparent;
  }
}

@keyframes siegu-pulse {
  0% {
    box-shadow: 0 0 0 0 color-mix(in srgb, rgb(var(--v-theme-error)) 60%, transparent);
  }
  70% {
    box-shadow: 0 0 0 7px transparent;
  }
  100% {
    box-shadow: 0 0 0 0 transparent;
  }
}
</style>
