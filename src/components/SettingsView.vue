<template>
  <v-container :class="embedded ? 'pa-0' : 'pb-16 pt-2'" fluid>
    <PageLoading v-if="settingsLoading" class="py-16" />
    <v-row v-else justify="center">
      <v-col cols="12" :md="embedded ? 12 : 8" :lg="embedded ? 12 : 6">
        <div v-if="!embedded" class="d-flex align-center justify-space-between mb-6">
          <div>
            <div class="d-flex align-center mb-1">
              <v-icon color="rgb(var(--v-theme-on-surface))" size="28" class="mr-3"
                >mdi-cog-outline</v-icon
              >
              <h1 class="text-h4 font-weight-bold text-high-emphasis">
                {{ $t('settings.title') }}
              </h1>
            </div>
            <div class="text-subtitle-1 text-medium-emphasis">{{ $t('settings.desc') }}</div>
          </div>
        </div>

        <FoldersSection
          v-if="!hideFolderSection"
          :directories="directories"
          @select-directory="selectDirectory"
          @remove-directory="removeDirectory"
          @remove-directory-full="openRemoveFolderFull"
        />

        <AiSection v-if="!embedded" />

        <AiSection v-if="embedded && !hideAiSection" :embedded="true" />

        <LanguageSection v-if="!embedded" :initial-lang="currentLang" />

        <AppearanceSection v-if="!embedded" :initial-theme="currentTheme" />

        <MaintenanceSection v-if="!embedded" />

        <StorageSection v-if="!embedded" />

        <SignallingSection
          v-if="!embedded"
          :model-value="signalingUrl"
          :token="signalingToken"
          :testing="signalingTesting"
          :saving="signallingSaving"
          :ping-result="signalingPingResult"
          @update:model-value="onSignallingUrl"
          @update:token="onSignallingToken"
          @test="testSignalling"
          @save="saveSignalling"
        />

        <UpdateSection
          v-if="!embedded && !isStoreManaged"
          :status="updateStatus"
          :status-text="updateStatusText"
          :btn-text="updateBtnText"
          :btn-icon="updateBtnIcon"
          :supported="updateSupported"
          @check-update="checkUpdate"
          @download-update="downloadUpdate"
        />

        <AboutSection v-if="!embedded" />
      </v-col>
    </v-row>

    <v-dialog v-model="cleanupDialog.show" max-width="400" rounded="xl">
      <v-card color="surface" border class="border overflow-hidden">
        <v-card-item class="py-4">
          <template v-slot:prepend>
            <v-avatar color="on-surface" size="32" class="mr-3">
              <v-icon color="surface" size="small">mdi-wrench-outline</v-icon>
            </v-avatar>
          </template>
          <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
            $t('settings.clear_db_title')
          }}</v-card-title>
          <template v-slot:append>
            <v-btn
              icon="mdi-close"
              variant="text"
              size="small"
              @click="cleanupDialog.show = false"
            ></v-btn>
          </template>
        </v-card-item>

        <v-card-text class="py-6 text-center">
          <div class="text-subtitle-1 text-medium-emphasis px-2">
            {{ $t('settings.clear_db_desc') }}
          </div>
        </v-card-text>

        <v-card-actions style="background: rgb(var(--v-theme-surface))" class="pa-4 border-t ga-2">
          <v-btn
            variant="flat"
            color="primary"
            @click="cleanupDialog.show = false"
            class="flex-grow-1"
            height="44"
            >{{ $t('settings.cancel') }}</v-btn
          >
          <v-btn
            variant="flat"
            color="error"
            @click="startConfirmedCleanup"
            class="flex-grow-1"
            height="44"
            >{{ $t('settings.clear') }}</v-btn
          >
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="removeFolderDialog.show" max-width="400" rounded="xl">
      <v-card color="surface" border class="border overflow-hidden">
        <v-card-item class="py-4">
          <template v-slot:prepend>
            <v-avatar color="on-surface" size="32" class="mr-3">
              <v-icon color="surface" size="small">mdi-folder-remove-outline</v-icon>
            </v-avatar>
          </template>
          <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
            $t('settings.wipe_title')
          }}</v-card-title>
          <template v-slot:append>
            <v-btn
              icon="mdi-close"
              variant="text"
              size="small"
              @click="removeFolderDialog.show = false"
            ></v-btn>
          </template>
        </v-card-item>

        <v-card-text class="py-6 text-center">
          <div class="text-subtitle-1 text-medium-emphasis px-2">
            <span v-html="$t('settings.wipe_desc')"></span>
          </div>
        </v-card-text>

        <v-card-actions style="background: rgb(var(--v-theme-surface))" class="pa-4 border-t ga-2">
          <v-btn
            variant="flat"
            color="primary"
            @click="removeFolderDialog.show = false"
            class="flex-grow-1"
            height="44"
            >{{ $t('settings.cancel') }}</v-btn
          >
          <v-btn
            variant="flat"
            color="primary"
            @click="startConfirmedRemoveFolder"
            class="flex-grow-1"
            height="44"
            >{{ $t('settings.wipe_data') }}</v-btn
          >
        </v-card-actions>
      </v-card>
    </v-dialog>

    <FolderPicker v-model="showFolderPicker" @select="onFolderSelected" />
    <v-snackbar v-model="snackbar.show" :timeout="3000" location="bottom" color="primary">
      <div class="d-flex align-center">
        <v-icon
          size="small"
          class="mr-3"
          :color="snackbar.error ? 'error' : 'rgb(var(--v-theme-on-primary))'"
          >{{ snackbar.error ? 'mdi-alert-circle' : 'mdi-check-circle' }}</v-icon
        >
        <span class="text-body-2">{{ snackbar.text }}</span>
      </div>
    </v-snackbar>
  </v-container>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useSettings } from '@/composables/useSettings';
import FolderPicker from './FolderPicker.vue';
import FoldersSection from './settings/FoldersSection.vue';
import AiSection from './settings/AiSection.vue';
import LanguageSection from './settings/LanguageSection.vue';
import AppearanceSection from './settings/AppearanceSection.vue';
import MaintenanceSection from './settings/MaintenanceSection.vue';
import StorageSection from './settings/StorageSection.vue';
import SignallingSection from './settings/SignallingSection.vue';
import UpdateSection from './settings/UpdateSection.vue';
import AboutSection from './settings/AboutSection.vue';
import PageLoading from './shared/PageLoading.vue';

defineProps<{
  embedded?: boolean;
  hideAiSection?: boolean;
  hideFolderSection?: boolean;
}>();

const emit = defineEmits<{
  'folder-added': [directories: unknown[]];
}>();

const {
  directories,
  showFolderPicker,
  snackbar,
  cleanupDialog,
  removeFolderDialog,
  updateStatus,
  updateStatusText,
  updateBtnText,
  updateBtnIcon,
  updateSupported,
  currentPlatform,
  signalingUrl,
  signalingToken,
  signalingTesting,
  signalingPingResult,
  init,
  stopClock,
  selectDirectory,
  removeDirectory,
  openRemoveFolderFull,
  startConfirmedRemoveFolder,
  onFolderSelected,
  saveSignallingConfig,
  testSignalling,
  checkUpdate,
  downloadUpdate,
  startConfirmedCleanup,
} = useSettings();

const currentLang = ref(localStorage.getItem('siegu_language') || 'en');
const currentTheme = ref(localStorage.getItem('siegu_theme') || 'system');

const isStoreManaged = computed(
  () => currentPlatform.value === 'android' || currentPlatform.value === 'ios',
);

const signallingSaving = ref(false);

const settingsLoading = ref(true);

function onSignallingUrl(v: string): void {
  signalingUrl.value = v;
}

function onSignallingToken(v: string): void {
  signalingToken.value = v;
}

async function saveSignalling(): Promise<void> {
  signallingSaving.value = true;
  try {
    await saveSignallingConfig();
  } finally {
    signallingSaving.value = false;
  }
}

onMounted(async () => {
  try {
    await init();
  } catch (e) {
    console.error('[Setting] init failed:', e);
  } finally {
    settingsLoading.value = false;
  }
  emit('folder-added', directories.value);
});

onUnmounted(() => {
  stopClock();
});
</script>
