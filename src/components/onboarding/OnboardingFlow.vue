<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useAppStore } from '@/stores/app';
import { useModelsStore } from '@/stores/models';
import { useUiStore } from '@/stores/ui';
import { markOnboardingComplete, addDirectory } from '@/services/tauri';
import SettingsView from '@/components/SettingsView.vue';
import ConnectView from '@/components/ConnectView.vue';
import GreetView from '@/components/GreetView.vue';

const { t } = useI18n();
const appStore = useAppStore();
const modelsStore = useModelsStore();
const uiStore = useUiStore();

const step = ref<'greet' | 'folders' | 'models' | 'sync' | 'finalize'>('greet');
const syncPath = ref('');
const showSyncPicker = ref(false);
const showOwnFolderPicker = ref(false);
const ownFolder = ref('');
const choosingOwnFolder = ref(false);
const connectionMode = ref<'host' | 'join'>('host');
const showConnectUI = ref(false);
const deviceConnected = ref(false);

async function goToStep(next: typeof step.value): Promise<void> {
  if (next === 'models') {
    await modelsStore.loadModels();
  }
  step.value = next;
}

function handleGreetLocal(): void {
  goToStep('folders');
}

function handleGreetSync(): void {
  connectionMode.value = 'join';
  showConnectUI.value = true;
  goToStep('sync');
}

async function handleSetSyncPath(path: string): Promise<void> {
  syncPath.value = path;
}

async function handlePickOwnFolder(path: string): Promise<void> {
  ownFolder.value = path;
  choosingOwnFolder.value = true;
  try {
    await addDirectory(path);
  } catch (error) {
    console.error('[Onboarding] addDirectory failed:', error);
  } finally {
    choosingOwnFolder.value = false;
  }
  await finishSetupAndScan(false);
}

async function finishSetupAndScan(showTour = true): Promise<void> {
  try {
    await markOnboardingComplete();
  } catch (error) {
    console.error('[Onboarding] markOnboardingComplete failed:', error);
  }
  appStore.completeOnboarding();
  uiStore.setPage('home');
  if (showTour) {
    setTimeout(() => appStore.startTour(), 1500);
  }
  setTimeout(() => appStore.startScan(), 500);
}
</script>

<template>
  <GreetView
    v-if="step === 'greet'"
    @setup-local="handleGreetLocal"
    @setup-sync="handleGreetSync"
  />

  <v-container v-else-if="step === 'folders'" class="fill-height" fluid>
    <v-row justify="center">
      <v-col cols="12" sm="10" md="8" lg="6">
        <v-card variant="flat" rounded="xl" class="pa-8 border">
          <div class="text-center mb-8">
            <v-avatar color="rgb(var(--v-theme-surface-light))" size="48" class="mx-auto mb-4">
              <v-icon color="rgb(var(--v-theme-on-surface))">mdi-folder-plus</v-icon>
            </v-avatar>
            <h2 class="text-h4 font-weight-bold text-high-emphasis">
              {{ t('onboarding.add_media_title') }}
            </h2>
            <p class="text-medium-emphasis">{{ t('onboarding.add_media_desc') }}</p>
          </div>
          <SettingsView :embedded="true" hide-ai-section />
          <v-btn block color="primary" height="56" class="mt-8" @click="goToStep('models')">
            {{ t('onboarding.continue_ai') }}
          </v-btn>
        </v-card>
      </v-col>
    </v-row>
  </v-container>

  <v-container v-else-if="step === 'models'" class="fill-height" fluid>
    <v-row justify="center">
      <v-col cols="12" sm="10" md="8" lg="6">
        <v-card variant="flat" rounded="xl" class="pa-8 border">
          <div class="text-center mb-8">
            <v-avatar color="on-surface" size="48" class="mx-auto mb-4">
              <v-icon color="surface">mdi-auto-fix</v-icon>
            </v-avatar>
            <h2 class="text-h4 font-weight-bold text-high-emphasis">
              {{ t('onboarding.ai_title') }}
            </h2>
            <p class="text-medium-emphasis">{{ t('onboarding.ai_desc') }}</p>
          </div>
          <SettingsView :embedded="true" hide-folder-section />
          <v-btn
            block
            color="primary"
            height="56"
            class="mt-8"
            :loading="modelsStore.downloading"
            :disabled="modelsStore.downloaded.length < 2 && !modelsStore.downloading"
            @click="goToStep('sync')"
          >
            {{
              modelsStore.downloaded.length < 2
                ? t('onboarding.download_required')
                : t('onboarding.continue')
            }}
          </v-btn>
        </v-card>
      </v-col>
    </v-row>
  </v-container>

  <v-container v-else-if="step === 'sync'" class="fill-height" fluid>
    <v-row justify="center">
      <v-col cols="12" sm="10" md="8" lg="6">
        <v-card variant="flat" rounded="xl" class="pa-8 border">
          <div class="text-center mb-8">
            <v-avatar color="rgb(var(--v-theme-surface-light))" size="48" class="mx-auto mb-4">
              <v-icon color="rgb(var(--v-theme-on-surface))">mdi-cellphone-link</v-icon>
            </v-avatar>
            <h2 class="text-h4 font-weight-bold text-high-emphasis">
              {{ t('onboarding.sync_title') }}
            </h2>
            <p class="text-medium-emphasis">{{ t('onboarding.sync_desc') }}</p>
          </div>

          <v-expand-transition>
            <div v-if="connectionMode === 'join' && !deviceConnected" class="mb-8">
              <v-card
                variant="flat"
                color="surface"
                class="border pa-4 rounded-xl d-flex align-center"
              >
                <v-icon color="rgba(var(--v-theme-on-surface), 0.7)" class="mr-3"
                  >mdi-folder-sync</v-icon
                >
                <div class="flex-grow-1 overflow-hidden">
                  <div class="text-caption text-medium-emphasis">
                    {{ t('onboarding.sync_storage') }}
                  </div>
                  <div class="text-body-2 font-weight-bold text-high-emphasis text-truncate">
                    {{ syncPath || t('onboarding.auto_select') }}
                  </div>
                </div>
                <v-btn
                  variant="flat"
                  size="small"
                  color="primary"
                  class="ml-4"
                  @click="showSyncPicker = true"
                >
                  {{ t('onboarding.change') }}
                </v-btn>
              </v-card>
            </div>
          </v-expand-transition>

          <FolderPicker v-model="showSyncPicker" @select="handleSetSyncPath" />

          <div class="d-flex justify-center mb-8">
            <ConnectView
              :embedded="true"
              :initial-mode="connectionMode"
              :hide-mode-toggle="true"
              @connected="deviceConnected = true"
              @mode-change="connectionMode = $event"
            />
          </div>

          <v-fade-transition>
            <div v-if="deviceConnected" class="mb-6">
              <div
                style="
                  background: rgba(var(--v-theme-success), 0.12);
                  border: 1px solid rgba(var(--v-theme-success), 0.3);
                "
                class="pa-4 rounded-xl mb-4 text-center d-flex align-center justify-center"
              >
                <v-icon color="success" class="mr-2">mdi-check-circle</v-icon>
                <span class="font-weight-bold" style="color: rgb(var(--v-theme-success))">{{
                  t('onboarding.device_linked')
                }}</span>
              </div>
            </div>
          </v-fade-transition>

          <v-fade-transition>
            <div v-if="deviceConnected && connectionMode === 'join'" class="mb-6">
              <v-card variant="flat" color="surface" class="border pa-4 rounded-xl">
                <div class="text-caption font-weight-bold text-high-emphasis mb-1">
                  {{ t('onboarding.your_library_title') }}
                </div>
                <div class="text-caption text-medium-emphasis mb-4">
                  {{ t('onboarding.your_library_desc') }}
                </div>
                <div class="d-flex flex-column ga-2">
                  <v-btn
                    block
                    color="primary"
                    height="48"
                    class=""
                    :loading="choosingOwnFolder"
                    @click="showOwnFolderPicker = true"
                  >
                    <v-icon start class="mr-2">mdi-folder-open</v-icon>
                    {{ ownFolder || t('onboarding.choose_folder') }}
                  </v-btn>
                  <v-btn
                    block
                    color="primary"
                    variant="tonal"
                    height="48"
                    class=""
                    @click="finishSetupAndScan(false)"
                  >
                    {{ t('onboarding.skip') }}
                  </v-btn>
                </div>
              </v-card>
            </div>
          </v-fade-transition>

          <FolderPicker v-model="showOwnFolderPicker" @select="handlePickOwnFolder" />

          <div class="d-flex flex-column ga-3">
            <v-btn
              v-if="!showConnectUI && !deviceConnected"
              block
              color="primary"
              height="56"
              class=""
              @click="showConnectUI = true"
            >
              <v-icon start class="mr-2">mdi-link-variant</v-icon>
              {{ t('onboarding.link_device') }}
            </v-btn>
            <v-btn
              v-if="!showConnectUI && !deviceConnected"
              block
              color="primary"
              height="56"
              class=""
              @click="goToStep('finalize')"
            >
              {{ t('onboarding.skip') }}
            </v-btn>
            <v-btn
              v-if="deviceConnected && connectionMode === 'host'"
              block
              color="primary"
              height="56"
              class=""
              @click="finishSetupAndScan"
            >
              <v-icon start class="mr-2">mdi-sync</v-icon>
              {{ t('onboarding.start_syncing') }}
            </v-btn>
            <v-btn
              v-if="showConnectUI && !deviceConnected"
              block
              color="primary"
              height="56"
              class=""
              @click="goToStep('finalize')"
            >
              {{ t('onboarding.skip') }}
            </v-btn>
          </div>
        </v-card>
      </v-col>
    </v-row>
  </v-container>

  <v-container v-else-if="step === 'finalize'" class="fill-height" fluid>
    <v-row justify="center">
      <v-col cols="12" sm="10" md="8" lg="6">
        <v-card variant="flat" rounded="xl" class="pa-8 border text-center">
          <div class="success-check-animation mb-8">
            <v-icon size="80" color="success">mdi-check-decagram</v-icon>
          </div>
          <h2 class="text-h3 font-weight-black text-high-emphasis mb-4">
            {{ t('onboarding.ready_title') }}
          </h2>
          <p class="text-body-1 text-medium-emphasis mb-10">{{ t('onboarding.ready_desc') }}</p>
          <v-btn block color="primary" height="64" class="mb-4" @click="finishSetupAndScan">
            <v-icon start class="mr-2">{{
              deviceConnected ? 'mdi-sync' : 'mdi-magnify-scan'
            }}</v-icon>
            {{ deviceConnected ? t('onboarding.finish_setup') : t('onboarding.start_scan') }}
          </v-btn>
          <div class="text-caption text-disabled">{{ t('onboarding.scan_desc') }}</div>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>
