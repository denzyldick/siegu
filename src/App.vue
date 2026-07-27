<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useScanStore } from '@/stores/scan'
import { useModelsStore } from '@/stores/models'
import { useUiStore } from '@/stores/ui'
import AppDock from '@/components/layout/AppDock.vue'
import AppToolbar from '@/components/layout/AppToolbar.vue'
import ProgressBanner from '@/components/layout/ProgressBanner.vue'
import SyncStatusBanner from '@/components/layout/SyncStatusBanner.vue'
import OnboardingFlow from '@/components/onboarding/OnboardingFlow.vue'
import MediaLibrary from '@/components/MediaLibrary.vue'
import People from '@/components/People.vue'
import Map from '@/components/Map.vue'
import DeviceList from '@/components/DeviceList.vue'
import Setting from '@/components/Setting.vue'
import GuidedTour from '@/components/GuidedTour.vue'
import ErrorBoundary from '@/components/shared/ErrorBoundary.vue'

const { t } = useI18n()
const appStore = useAppStore()
const scanStore = useScanStore()
const modelsStore = useModelsStore()
const uiStore = useUiStore()

const currentPage = computed(() => uiStore.currentPage)

onMounted(async () => {
  try {
    await appStore.checkInitialized()
  } catch (e) {
    console.error('[App] checkInitialized failed:', e)
  }

  try {
    await appStore.detectOs()
  } catch (e) {
    console.error('[App] detectOs failed:', e)
  }

  if (!appStore.isNewInstall) {
    try {
      await appStore.loadDirectories()
    } catch (e) {
      console.error('[App] loadDirectories failed:', e)
    }
    try {
      await appStore.loadLastScanTime()
    } catch (e) {
      console.error('[App] loadLastScanTime failed:', e)
    }
    try {
      await scanStore.loadIndexingStatus()
    } catch (e) {
      console.error('[App] loadIndexingStatus failed:', e)
    }
    try {
      await scanStore.loadUnindexedCount()
    } catch (e) {
      console.error('[App] loadUnindexedCount failed:', e)
    }
    try {
      await modelsStore.loadModels()
    } catch (e) {
      console.error('[App] loadModels failed:', e)
    }
  }
})

function handleSearch(_query: string): void {
  uiStore.setPage('home')
}

function handleClearSearch(): void {
  // Handled by AppToolbar internally
}
</script>

<template>
  <v-app class="bg-siegu-main">
    <template v-if="appStore.isNewInstall">
      <ErrorBoundary>
        <OnboardingFlow />
      </ErrorBoundary>
    </template>

    <template v-else>
      <AppToolbar
        :is-active="scanStore.isActive"
        :status-label="scanStore.isActive ? t('sync.indexing') : t('sync.refresh')"
        @scan="appStore.startScan()"
        @search="handleSearch"
      />

      <v-main class="bg-siegu-main">
        <ProgressBanner />
        <ErrorBoundary>
          <div data-tour="photos" class="w-100">
            <MediaLibrary
              v-if="currentPage === 'home'"
              :search-query="''"
              :filters="{ favoritesOnly: false, videosOnly: false, dateRange: 'all', folder: null }"
              @clear-search="handleClearSearch"
            />
          </div>
          <People v-if="currentPage === 'people'" />
          <Map v-if="currentPage === 'location'" />
          <DeviceList v-if="currentPage === 'devices'" />
          <Setting v-if="currentPage === 'settings'" @done="uiStore.setPage('home')" />
        </ErrorBoundary>
      </v-main>

      <AppDock />
    </template>

    <SyncStatusBanner />
    <GuidedTour :active="appStore.showTour" @finish="appStore.dismissTour()" @skip="appStore.dismissTour()" />
  </v-app>
</template>
