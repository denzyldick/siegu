<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useUiStore } from '@/stores/ui'
import { useScanStore } from '@/stores/scan'
import { normalizeIndexingCount, formatEta } from '@/composables/useMediaUtils'
import logo from '@/assets/logo.png'

const { t } = useI18n()
const uiStore = useUiStore()
const scanStore = useScanStore()

const navItems = [
  { page: 'home' as const, icon: null, tour: 'dock-home', useLogo: true },
  { page: 'albums' as const, icon: 'mdi-image-album', tour: 'dock-albums', useLogo: false },
  { page: 'people' as const, icon: 'mdi-account-group-outline', tour: 'dock-people', useLogo: false },
  { page: 'location' as const, icon: 'mdi-map-outline', tour: 'dock-map', useLogo: false },
  { page: 'devices' as const, icon: 'mdi-laptop', tour: 'dock-devices', useLogo: false },
  { page: 'settings' as const, icon: 'mdi-cog-outline', tour: 'dock-settings', useLogo: false },
]

const isIndexing = computed(() => scanStore.isActive)
const jobsLeft = computed(() => normalizeIndexingCount(scanStore.indexingCount))
const statusLabel = computed(() => {
  if (!scanStore.isActive) return ''
  if (scanStore.status === 'scanning') return t('sync.scanning')
  if (scanStore.indexingCount > 0) {
    return t('sync.indexing')
  }
  return t('sync.indexing')
})

const tooltipText = computed(() => {
  if (!scanStore.isActive) return ''
  const eta = scanStore.indexingEta && scanStore.indexingEta > 0 ? formatEta(scanStore.indexingEta) : null
  if (scanStore.indexingCount > 0) {
    return eta
      ? `${statusLabel.value}: ${t('sync.jobs_left', { count: jobsLeft.value.toLocaleString() })} · ~${eta}`
      : `${statusLabel.value}: ${t('sync.jobs_left', { count: jobsLeft.value.toLocaleString() })}`
  }
  return statusLabel.value
})

function navigate(page: 'home' | 'albums' | 'people' | 'location' | 'devices' | 'settings'): void {
  uiStore.setPage(page)
}
</script>

<template>
  <div class="dock-container">
    <v-sheet
      class="dock d-flex justify-space-around align-center pa-2 rounded-pill mb-8"
      elevation="0"
      width="100%"
      max-width="380"
      color="surface"
    >
      <template v-for="item in navItems" :key="item.page">
        <v-tooltip
          v-if="item.useLogo"
          :text="isIndexing ? tooltipText : ''"
          location="top"
          :disabled="!isIndexing"
        >
          <template v-slot:activator="{ props }">
            <v-btn
              v-bind="props"
              icon
              variant="text"
              size="small"
              class="siegu-dock-btn"
              :class="{ 'siegu-dock-btn--active': uiStore.currentPage === item.page }"
              :data-tour="item.tour"
              @click="navigate(item.page)"
            >
            <div class="siegu-logo-wrap">
              <v-img
                :src="logo"
                width="24"
                height="24"
                :class="uiStore.currentPage === item.page ? 'siegu-logo--active' : 'opacity-40'"
              />
              <template v-if="isIndexing">
                <span class="indexing-dot" aria-label="indexing"></span>
                <span v-if="jobsLeft > 0" class="indexing-pill">{{ jobsLeft.toLocaleString() }}</span>
              </template>
            </div>
            </v-btn>
          </template>
        </v-tooltip>

        <v-btn
          v-else
          icon
          variant="text"
          size="small"
          class="siegu-dock-btn"
          :class="{ 'siegu-dock-btn--active': uiStore.currentPage === item.page }"
          :data-tour="item.tour"
          @click="navigate(item.page)"
        >
          <v-icon size="24">{{ item.icon }}</v-icon>
        </v-btn>
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
}

.dock {
  pointer-events: auto;
  backdrop-filter: blur(16px);
  border: 1px solid var(--color-border-default);
}

.siegu-dock-btn {
  color: var(--color-text-muted) !important;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1) !important;
  border-radius: 50% !important;
}

.siegu-dock-btn:hover {
  background: var(--color-bg-hover) !important;
  color: var(--color-text-primary) !important;
  transform: translateY(-2px);
}

.siegu-dock-btn--active {
  color: var(--color-text-btn) !important;
  background: var(--color-bg-btn) !important;
}

.siegu-logo--active {
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
  background: #ef4444;
  animation: siegu-pulse 1.6s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

.indexing-pill {
  position: absolute;
  bottom: -5px;
  right: -10px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 999px;
  background: #ef4444;
  color: #fff;
  font-size: 9px;
  font-weight: 800;
  line-height: 16px;
  text-align: center;
  border: 2px solid var(--color-bg-surface);
}

@keyframes siegu-pulse {
  0% {
    box-shadow: 0 0 0 0 rgba(239, 68, 68, 0.6);
  }
  70% {
    box-shadow: 0 0 0 7px rgba(239, 68, 68, 0);
  }
  100% {
    box-shadow: 0 0 0 0 rgba(239, 68, 68, 0);
  }
}
</style>
