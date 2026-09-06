<template>
  <v-container :class="embedded ? 'pa-0' : 'pb-16 pt-2'" fluid>
    <PageLoading v-if="settingsLoading" class="py-16" />
    <v-row v-else justify="center">
      <v-col cols="12" :md="embedded ? 12 : 8" :lg="embedded ? 12 : 6">
        <div
          v-if="!embedded"
          class="d-flex align-center justify-space-between mb-6"
          data-tour="settings-help"
        >
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
          <v-tooltip location="bottom">
            <template #activator="{ props: tipProps }">
              <v-btn
                v-bind="tipProps"
                icon
                variant="text"
                :aria-label="$t('guided_tour.help')"
                data-tour="settings-help-trigger"
                @click="emit('start-tour')"
              >
                <v-icon>mdi-help-circle-outline</v-icon>
              </v-btn>
            </template>
            <span>{{ $t('guided_tour.help') }}</span>
          </v-tooltip>
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
        <MapSection v-if="!embedded" />

        <ProSection
          v-if="!embedded"
          :email="proEmail"
          :sending="proSending"
          :verifying="proVerifying"
          :saving="proSaving"
          :result="proStatus"
          :license-url="proLicenseUrl"
          :pro-url="APP_PRO_URL"
          :signalling-url="signalingUrl"
          :signalling-token="signalingToken"
          :signalling-testing="signalingTesting"
          :signalling-saving="signallingSaving"
          :signal-ping-result="signalingPingResult"
          :turn-enabled="turnEnabled"
          :turn-port="turnPort"
          :turn-public-host="turnPublicHost"
          :turn-saving="turnSaving"
          :connect-url="APP_CONNECT_URL"
          @update:email="onProEmail"
          @update:license-url="onProLicenseUrl"
          @update:signalling-url="onSignallingUrl"
          @update:signalling-token="onSignallingToken"
          @update:turn-enabled="onTurnEnabled"
          @update:turn-port="onTurnPort"
          @update:turn-public-host="onTurnPublicHost"
          @verify="startProVerification"
          @close-verify-dialog="closeProDialog"
          :verify-dialog="proDialogOpen"
          :verify-dialog-sending="proDialogSending"
          :verify-dialog-verifying="proDialogVerifying"
          @save-config="saveLicense"
          @test-signalling="testSignalling"
          @save-signalling="saveSignalling"
          @save-turn="saveTurn"
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
            <v-avatar color="surface" size="32" class="mr-3">
              <v-icon color="on-surface" size="small">mdi-wrench-outline</v-icon>
            </v-avatar>
          </template>
          <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
            cleanupConfirming ? $t('settings.clear_db_title') : $t('settings.clear_db_title')
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
          <div v-if="!cleanupConfirming" class="text-subtitle-1 text-medium-emphasis px-2">
            {{ $t('settings.clear_db_desc') }}
          </div>
          <div v-else class="text-subtitle-1 px-2" style="color: rgb(var(--v-theme-error))">
            {{ $t('settings.clear_db_confirm') }}
          </div>
        </v-card-text>

        <v-card-actions style="background: rgb(var(--v-theme-surface))" class="pa-4 border-t ga-2">
          <v-btn
            variant="flat"
            color="primary"
            @click="cleanupDialog.show = false"
            class="flex-grow-1"
            height="44"
          >
            <v-icon start size="18">mdi-close</v-icon>
            {{ $t('settings.cancel') }}
          </v-btn>
          <v-btn
            variant="flat"
            :color="cleanupConfirming ? 'error' : 'primary'"
            @click="cleanupConfirming ? startConfirmedCleanup() : (cleanupConfirming = true)"
            class="flex-grow-1"
            height="44"
          >
            <v-icon start size="18">{{
              cleanupConfirming ? 'mdi-alert' : 'mdi-delete-outline'
            }}</v-icon>
            {{ cleanupConfirming ? $t('settings.clear_db_are_you_sure') : $t('settings.clear') }}
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="removeFolderDialog.show" max-width="400" rounded="xl">
      <v-card color="surface" border class="border overflow-hidden">
        <v-card-item class="py-4">
          <template v-slot:prepend>
            <v-avatar color="surface" size="32" class="mr-3">
              <v-icon color="on-surface" size="small">mdi-folder-remove-outline</v-icon>
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
          <div v-if="!wipeConfirming" class="text-subtitle-1 text-medium-emphasis px-2">
            <span v-html="$t('settings.wipe_desc')"></span>
          </div>
          <div v-else class="text-subtitle-1 px-2" style="color: rgb(var(--v-theme-error))">
            {{ $t('settings.wipe_confirm') }}
          </div>
        </v-card-text>

        <v-card-actions style="background: rgb(var(--v-theme-surface))" class="pa-4 border-t ga-2">
          <v-btn
            variant="flat"
            color="primary"
            @click="removeFolderDialog.show = false"
            class="flex-grow-1"
            height="44"
          >
            <v-icon start size="18">mdi-close</v-icon>
            {{ $t('settings.cancel') }}
          </v-btn>
          <v-btn
            variant="flat"
            :color="wipeConfirming ? 'error' : 'primary'"
            @click="wipeConfirming ? startConfirmedRemoveFolder() : (wipeConfirming = true)"
            class="flex-grow-1"
            height="44"
          >
            <v-icon start size="18">{{
              wipeConfirming ? 'mdi-alert' : 'mdi-folder-remove-outline'
            }}</v-icon>
            {{ wipeConfirming ? $t('settings.wipe_are_you_sure') : $t('settings.wipe_data') }}
          </v-btn>
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
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { useSettings } from '@/composables/useSettings';
import FolderPicker from './FolderPicker.vue';
import FoldersSection from './settings/FoldersSection.vue';
import AiSection from './settings/AiSection.vue';
import LanguageSection from './settings/LanguageSection.vue';
import AppearanceSection from './settings/AppearanceSection.vue';
import MaintenanceSection from './settings/MaintenanceSection.vue';
import StorageSection from './settings/StorageSection.vue';
import MapSection from './settings/MapSection.vue';
import ProSection from './settings/ProSection.vue';
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
  'start-tour': [];
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
  turnEnabled,
  turnPort,
  turnPublicHost,
  saveTurnConfig,
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
  proEmail,
  proSending,
  proVerifying,
  proStatus,
  proLicenseUrl,
  APP_PRO_URL,
  APP_CONNECT_URL,
  startProVerification,
  closeProDialog,
  proDialogOpen,
  proDialogSending,
  proDialogVerifying,
  saveLicenseConfig,
} = useSettings();

const currentLang = ref(localStorage.getItem('siegu_language') || 'en');
const currentTheme = ref(localStorage.getItem('siegu_theme') || 'system');

const isStoreManaged = computed(
  () => currentPlatform.value === 'android' || currentPlatform.value === 'ios',
);

const signallingSaving = ref(false);
const proSaving = ref(false);
const turnSaving = ref(false);
const cleanupConfirming = ref(false);
const wipeConfirming = ref(false);

watch(
  () => cleanupDialog.show,
  (v) => {
    if (!v) cleanupConfirming.value = false;
  },
);
watch(
  () => removeFolderDialog.show,
  (v) => {
    if (!v) wipeConfirming.value = false;
  },
);

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

function onProEmail(v: string): void {
  proEmail.value = v;
}
function onProLicenseUrl(v: string): void {
  proLicenseUrl.value = v;
}

function onTurnEnabled(v: boolean): void {
  turnEnabled.value = v;
}
function onTurnPort(v: string): void {
  turnPort.value = v;
}
function onTurnPublicHost(v: string): void {
  turnPublicHost.value = v;
}

async function saveTurn(): Promise<void> {
  turnSaving.value = true;
  try {
    await saveTurnConfig();
  } finally {
    turnSaving.value = false;
  }
}

async function saveLicense(): Promise<void> {
  proSaving.value = true;
  try {
    await saveLicenseConfig();
  } finally {
    proSaving.value = false;
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
  closeProDialog();
});
</script>
