<script setup lang="ts">
import { onMounted, computed, ref } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { useI18n } from 'vue-i18n';
import { useAppStore } from '@/stores/app';
import { useScanStore } from '@/stores/scan';
import { useModelsStore } from '@/stores/models';
import { useUiStore } from '@/stores/ui';
import { useSyncStore } from '@/stores/sync';
import { useSearchStore } from '@/stores/search';
import { useRuntimeStore } from '@/stores/runtime';
import { autoReconnect, discoverLanDevices } from '@/services/tauri';
import AppDock from '@/components/layout/AppDock.vue';
import AppToolbar from '@/components/layout/AppToolbar.vue';
import SyncStatusBanner from '@/components/layout/SyncStatusBanner.vue';
import OnboardingFlow from '@/components/onboarding/OnboardingFlow.vue';
import MediaLibrary from '@/components/MediaLibrary.vue';
import CollectionsView from '@/components/CollectionsView.vue';
import MapView from '@/components/MapView.vue';
import DeviceList from '@/components/DeviceList.vue';
import SettingsView from '@/components/SettingsView.vue';
import GuidedTour from '@/components/GuidedTour.vue';
import ErrorBoundary from '@/components/shared/ErrorBoundary.vue';
import PersonMatchControls from '@/components/search/PersonMatchControls.vue';
import ScanExperience from '@/components/ScanExperience.vue';
import ProgressBanner from '@/components/layout/ProgressBanner.vue';
import { settingsTourSteps } from '@/components/GuidedTourSteps';

const { t } = useI18n();
const appStore = useAppStore();
const scanStore = useScanStore();
const modelsStore = useModelsStore();
const uiStore = useUiStore();

const currentPage = computed(() => uiStore.currentPage);
const searchStore = useSearchStore();

const mediaFilters = computed(() => ({
  favoritesOnly: searchStore.mediaFilters.favoritesOnly,
  videosOnly: searchStore.mediaFilters.videosOnly,
  facesOnly: searchStore.mediaFilters.facesOnly,
  papersOnly: searchStore.mediaFilters.papersOnly,
  nsfwOnly: searchStore.mediaFilters.nsfwOnly,
  camera: searchStore.camera,
  aestheticsMin: searchStore.aestheticsMin,
  surprise: searchStore.surprise,
  orderBy: searchStore.sortOrder,
  personMatch: searchStore.personMatch,
  personAlone: searchStore.personAlone,
  dateRange: 'all',
  folder: null,
}));

const runtimeStore = useRuntimeStore();

const openedFile = ref<string | null>(null);
const fileSnackbar = ref(false);

function openedFileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

onMounted(async () => {
  await runtimeStore.initRuntime();

  if (runtimeStore.isDesktop) {
    listen<string>('file-opened', (event) => {
      openedFile.value = event.payload;
      fileSnackbar.value = true;
      uiStore.setPage('home');
    });
  }

  // In browser modes there is no local Tauri library to interrogate:
  //  - webHost: the mounted host library behind `/session` is already initialized
  //  - guest:   media is streamed from the remote Siegu once paired
  //  - onboarding: nothing initialized yet
  // Only `tauri` (desktop) runs the local boot sequence (which on failure would
  // otherwise trip `isNewInstall` and show the desktop OnboardingFlow in a browser).
  if (!runtimeStore.isDesktop) {
    appStore.completeOnboarding();
    return;
  }

  try {
    await appStore.checkInitialized();
  } catch (e) {
    console.error('[App] checkInitialized failed:', e);
  }

  try {
    await appStore.detectOs();
  } catch (e) {
    console.error('[App] detectOs failed:', e);
  }

  if (!appStore.isNewInstall) {
    try {
      await appStore.loadDirectories();
    } catch (e) {
      console.error('[App] loadDirectories failed:', e);
    }
    try {
      await appStore.loadLastScanTime();
    } catch (e) {
      console.error('[App] loadLastScanTime failed:', e);
    }
    try {
      await scanStore.loadIndexingStatus();
    } catch (e) {
      console.error('[App] loadIndexingStatus failed:', e);
    }
    try {
      await scanStore.loadUnindexedCount();
    } catch (e) {
      console.error('[App] loadUnindexedCount failed:', e);
    }
    try {
      await modelsStore.loadModels();
    } catch (e) {
      console.error('[App] loadModels failed:', e);
    }
  }

  if (!appStore.isNewInstall) {
    void tryAutoReconnect();
  }
});

async function tryAutoReconnect(): Promise<void> {
  const syncStore = useSyncStore();
  try {
    if (syncStore.connection === 'connected') return;
    // Discover the host first: on Android this goes through NsdManager (raw
    // multicast in Rust is unreliable there), elsewhere through the Rust mDNS
    // path. The result is only a preferred URL — auto_reconnect still falls
    // back to mDNS and the saved session address.
    let hint: string | null = null;
    try {
      const hosts = await discoverLanDevices(2);
      const host = hosts[0];
      if (host?.ip && host.port) hint = `ws://${host.ip}:${host.port}`;
    } catch {
      // discovery is best-effort; saved/mDNS candidates still apply
    }
    await autoReconnect(hint);
  } catch (e) {
    console.error('[App] autoReconnect failed:', e);
  }
}

function handleClearSearch(): void {
  searchStore.clearQuery();
  searchStore.clearFilters();
}

function handleSearchPerson(person: { id: string | number; name: string }): void {
  searchStore.addFilter({ type: 'person', value: String(person.id), label: person.name });
  searchStore.clearQuery();
  uiStore.setPage('home');
}

function removeFilterChip(index: number): void {
  const filter = searchStore.activeFilters[index];
  if (filter) {
    searchStore.removeFilterValue(filter.type, filter.value);
  }
}
</script>

<template>
  <v-app>
    <template v-if="appStore.isNewInstall">
      <ErrorBoundary>
        <OnboardingFlow />
      </ErrorBoundary>
    </template>

    <template v-else>
      <ScanExperience />

      <AppToolbar v-if="currentPage === 'home'" />

      <v-main>
        <ErrorBoundary :key="currentPage">
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
                <v-icon start size="16">{{
                  filter.type === 'person'
                    ? 'mdi-account'
                    : filter.type === 'location'
                      ? 'mdi-map-marker'
                      : filter.type === 'tag'
                        ? 'mdi-tag'
                        : 'mdi-calendar-month'
                }}</v-icon>
                {{ filter.label }}
              </v-chip>
              <PersonMatchControls v-if="searchStore.personCount > 0" />
              <v-btn size="x-small" variant="text" @click="handleClearSearch">
                {{ t('search.clear_all') }}
              </v-btn>
            </div>
            <MediaLibrary
              v-if="currentPage === 'home'"
              :search-query="searchStore.query"
              :facets="searchStore.activeFilters"
              :filters="mediaFilters"
              @clear-search="handleClearSearch"
              @search-person="handleSearchPerson"
            />
          </div>
          <CollectionsView v-if="currentPage === 'collections'" />
          <MapView v-if="currentPage === 'location'" />
          <DeviceList v-if="currentPage === 'devices'" />
          <SettingsView
            v-if="currentPage === 'settings'"
            @done="uiStore.setPage('home')"
            @start-tour="appStore.startSettingsTour()"
          />
        </ErrorBoundary>
      </v-main>

      <ProgressBanner />
      <AppDock />
    </template>

    <SyncStatusBanner />
    <v-snackbar v-model="fileSnackbar" timeout="5000" color="surface" location="bottom">
      <div class="d-flex align-center ga-2">
        <v-icon color="primary" size="20">mdi-file-image-outline</v-icon>
        <span>
          {{
            t('open_with.opened_in_siegu', { file: openedFile ? openedFileName(openedFile) : '' })
          }}
        </span>
      </div>
    </v-snackbar>
    <GuidedTour
      :active="appStore.showTour && !scanStore.showFullScreen"
      @finish="appStore.dismissTour()"
      @skip="appStore.dismissTour()"
    />
    <GuidedTour
      :active="appStore.settingsShowTour && !scanStore.showFullScreen"
      :steps="settingsTourSteps"
      @finish="appStore.dismissSettingsTour()"
      @skip="appStore.dismissSettingsTour()"
    />
  </v-app>
</template>
