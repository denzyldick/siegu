<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useScanStore } from '@/stores/scan'
import { useModelsStore } from '@/stores/models'
import { useUiStore } from '@/stores/ui'
import { useSyncStore } from '@/stores/sync'
import { useSearchStore } from '@/stores/search'
import { autoReconnect } from '@/services/tauri'
import AppDock from '@/components/layout/AppDock.vue'
import AppToolbar from '@/components/layout/AppToolbar.vue'
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
const searchStore = useSearchStore()

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

  if (!appStore.isNewInstall) {
    void tryAutoReconnect()
  }
})

async function tryAutoReconnect(): Promise<void> {
  const syncStore = useSyncStore()
  try {
    if (syncStore.connection === 'connected') return
    await autoReconnect()
  } catch (e) {
    console.error('[App] autoReconnect failed:', e)
  }
}

function handleClearSearch(): void {
  searchStore.clearQuery()
  searchStore.clearFilters()
}

function handleSearchPerson(person: { id: string | number; name: string }): void {
  searchStore.addFilter({ type: 'person', value: String(person.id), label: person.name })
  searchStore.clearQuery()
  uiStore.setPage('home')
}

function removeFilterChip(index: number): void {
  const filter = searchStore.activeFilters[index]
  if (filter) {
    searchStore.removeFilter(filter.type)
  }
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
      <AppToolbar v-if="currentPage === 'home'" />

      <v-main class="bg-siegu-main">
        <ErrorBoundary>
          <div data-tour="photos" class="w-100">
            <div
              v-if="currentPage === 'home' && searchStore.activeFilters.length"
              class="d-flex align-center flex-wrap px-4 pt-2 ga-2"
              style="max-width: 980px; margin: 0 auto"
            >
              <v-chip
                v-for="(filter, index) in searchStore.activeFilters"
                :key="`${filter.type}-${filter.value}`"
                closable
                variant="tonal"
                @click:close="removeFilterChip(index)"
              >
                <v-icon start size="16">{{ filter.type === 'person' ? 'mdi-account' : filter.type === 'location' ? 'mdi-map-marker' : filter.type === 'tag' ? 'mdi-tag' : 'mdi-calendar-month' }}</v-icon>
                {{ filter.label || t('people.unnamed') }}
              </v-chip>
              <v-btn size="x-small" variant="text" @click="handleClearSearch">
                {{ t('search.clear_all') }}
              </v-btn>
            </div>
            <MediaLibrary
              v-if="currentPage === 'home'"
              :search-query="searchStore.query"
              :facets="searchStore.activeFilters"
              :filters="{
                favoritesOnly: searchStore.mediaFilters.favoritesOnly,
                videosOnly: searchStore.mediaFilters.videosOnly,
                facesOnly: searchStore.mediaFilters.facesOnly,
                papersOnly: searchStore.mediaFilters.papersOnly,
                camera: searchStore.camera,
                aestheticsMin: searchStore.aestheticsMin,
                surprise: searchStore.surprise,
                dateRange: 'all',
                folder: null,
              }"
              @clear-search="handleClearSearch"
              @search-person="handleSearchPerson"
            />
          </div>
          <People v-if="currentPage === 'people'" @search-person="handleSearchPerson" />
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
