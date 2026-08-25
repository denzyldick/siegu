<template>
  <v-container class="pa-6">
    <div class="d-flex align-center justify-space-between mb-8">
      <div>
        <div class="d-flex align-center mb-1">
          <v-icon color="rgb(var(--v-theme-on-surface))" size="28" class="mr-3">mdi-devices</v-icon>
          <h1 class="text-h4 font-weight-bold text-high-emphasis">{{ $t('devices.title') }}</h1>
        </div>
        <div class="text-subtitle-1 text-medium-emphasis d-none d-sm-block">{{ $t('devices.desc') }}</div>
      </div>
      <ConnectView />
    </div>

    <!-- Loading State -->
    <PageLoading v-if="loading" />

    <!-- Empty State -->
    <div
      v-else-if="devices.length === 0"
      class="d-flex flex-column align-center justify-center py-16 text-center animate-fade-in"
    >
      <v-icon size="64" color="rgba(var(--v-theme-on-surface), 0.25)" class="mb-4"
        >mdi-laptop-off</v-icon
      >
      <div class="text-h6 text-medium-emphasis font-weight-bold">
        {{ $t('devices.no_devices') }}
      </div>
      <p class="text-body-2 text-disabled mt-1 max-w-400 mx-auto">
        {{ $t('devices.no_devices_desc') }}
      </p>
    </div>

    <v-row v-else>
      <v-col cols="12" sm="6" md="4" v-for="device in devices" :key="device.title">
        <v-card
          variant="flat"
          height="100%"
          class="device-card border ga-2"
          rounded="xl"
          color="surface"
        >
          <v-card-item class="py-4">
            <template v-slot:prepend>
              <v-avatar color="surface" size="32" class="mr-3 device-icon-wrap">
                <v-icon color="on-surface" size="small">{{ device.icon }}</v-icon>
                <span
                  v-if="!device.host"
                  class="device-status-dot"
                  :class="dotClass"
                  :title="dotTitle"
                ></span>
              </v-avatar>
            </template>
            <v-card-title
              class="text-high-emphasis text-subtitle-1 font-weight-bold d-flex align-center"
            >
              {{ device.title }}
            </v-card-title>
            <v-card-subtitle class="text-medium-emphasis text-caption">{{
              device.host
                ? $t('devices.local_environment')
                : device.subtitle || $t('devices.connected')
            }}</v-card-subtitle>

            <template v-slot:append>
              <div class="d-flex align-center ga-1">
                <v-menu v-if="!device.host">
                  <template v-slot:activator="{ props }">
                    <v-btn
                      icon="mdi-dots-vertical"
                      variant="text"
                      size="small"
                      v-bind="props"
                      class="text-disabled"
                    ></v-btn>
                  </template>
                  <v-list density="compact" rounded="lg" class="border">
                    <v-list-item @click="openRename(device)">
                      <template v-slot:prepend>
                        <v-icon size="small">mdi-pencil-outline</v-icon>
                      </template>
                      <v-list-item-title>{{ $t('devices.rename_device') }}</v-list-item-title>
                    </v-list-item>
                    <v-list-item @click="removeDevice(device.id)" color="error">
                      <template v-slot:prepend>
                        <v-icon size="small" color="error">mdi-delete-outline</v-icon>
                      </template>
                      <v-list-item-title
                        style="color: rgb(var(--v-theme-error))"
                        class="font-weight-bold"
                        >{{ $t('devices.remove_device') }}</v-list-item-title
                      >
                    </v-list-item>
                  </v-list>
                </v-menu>
              </div>
            </template>
          </v-card-item>

          <v-card-text class="pt-0">
            <!-- Device Details -->
            <div
              style="background: rgb(var(--v-theme-surface))"
              class="rounded-lg pa-3 mb-4 border"
            >
              <div class="d-flex align-center mb-2">
                <v-icon size="14" color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2"
                  >mdi-desktop-tower-monitor</v-icon
                >
                <span
                  class="text-caption text-medium-emphasis font-weight-bold uppercase tracking-wider"
                  >{{ $t('devices.system') }}</span
                >
                <v-spacer></v-spacer>
                <span class="text-caption text-high-emphasis font-weight-bold capitalize">{{
                  device.os
                }}</span>
              </div>
              <div class="d-flex align-center mb-2">
                <v-icon size="14" color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2"
                  >mdi-image-multiple-outline</v-icon
                >
                <span class="text-caption text-medium-emphasis">{{
                  $t('media.label_photos')
                }}</span>
                <v-spacer></v-spacer>
                <span class="text-caption text-high-emphasis font-weight-bold">{{
                  device.remote_photo_count || device.photo_count
                }}</span>
              </div>
              <div class="d-flex align-center">
                <v-icon size="14" color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2"
                  >mdi-video-outline</v-icon
                >
                <span class="text-caption text-medium-emphasis">{{
                  $t('media.label_videos')
                }}</span>
                <v-spacer></v-spacer>
                <span class="text-caption text-high-emphasis font-weight-bold">{{
                  device.remote_video_count || device.video_count
                }}</span>
              </div>
            </div>

          </v-card-text>

          <v-card-actions class="px-4 pb-4 pt-0">
            <v-chip
              v-if="device.host"
              size="x-small"
              variant="outlined"
              color="primary"
              class="font-weight-bold mr-auto"
              >{{ $t('devices.this_device') }}</v-chip
            >
            <div v-if="device.syncing" class="w-100">
              <div class="d-flex align-center justify-space-between mb-1">
                <span class="text-caption text-disabled text-truncate mr-2">{{
                  device.syncStatus
                }}</span>
                <span
                  class="text-caption text-high-emphasis font-weight-bold"
                  v-if="device.items_total > 0"
                >
                  {{ device.items_completed }}/{{ device.items_total }}
                </span>
              </div>
              <v-progress-linear
                :model-value="device.progress"
                color="rgb(var(--v-theme-on-surface))"
                height="6"
                rounded
              ></v-progress-linear>
            </div>
            <div v-else class="d-flex align-center w-100">
              <v-btn
                v-if="!device.host && connection === 'connected'"
                variant="flat"
                color="primary"
                class="flex-grow-1"
                size="small"
                @click="startSync"
              >
                <v-icon start size="small">mdi-sync</v-icon>
                {{ $t('devices.sync_now') }}
              </v-btn>
              <v-btn
                v-else-if="!device.host && connection === 'offline'"
                variant="tonal"
                color="primary"
                class="flex-grow-1"
                size="small"
                :loading="reconnecting"
                @click="reconnect"
              >
                <v-icon start size="small">mdi-wifi-refresh</v-icon>
                {{ $t('devices.reconnect') }}
              </v-btn>
              <v-chip
                v-else
                size="x-small"
                color="success"
                variant="flat"
                class="text-white text-none border"
              >
                {{ $t('devices.online') }}
              </v-chip>
            </div>
          </v-card-actions>
        </v-card>
      </v-col>
    </v-row>

    <!-- Delete Confirmation Dialog -->
    <v-dialog v-model="deleteDialog" max-width="400" rounded="xl">
      <v-card class="pa-6 border">
        <v-avatar color="error" size="48" class="mb-4">
          <v-icon color="white">mdi-alert-outline</v-icon>
        </v-avatar>
        <v-card-title class="text-h5 font-weight-bold text-high-emphasis px-0 pb-2">{{
          $t('devices.remove_device_title')
        }}</v-card-title>
        <v-card-text class="text-medium-emphasis px-0 pb-6">
          <span>{{ $t('devices.remove_device_confirm', { name: deviceToDelete }) }}</span>
        </v-card-text>
        <v-card-actions class="px-0 ga-3">
          <v-btn
            variant="text"
            class="flex-grow-1 text-high-emphasis"
            height="44"
            @click="deleteDialog = false"
          >
            {{ $t('common.cancel') }}
          </v-btn>
          <v-btn
            variant="flat"
            color="error"
            class="flex-grow-1"
            height="44"
            @click="confirmDelete"
            :loading="deleting"
          >
            {{ $t('common.remove') }}
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Rename Dialog -->
    <v-dialog v-model="renameDialog" max-width="400" rounded="xl">
      <v-card class="pa-6 border">
        <v-avatar color="on-surface" size="48" class="mb-4">
          <v-icon color="surface">mdi-pencil-outline</v-icon>
        </v-avatar>
        <v-card-title class="text-h5 font-weight-bold text-high-emphasis px-0 pb-2">{{
          $t('devices.rename_device_title')
        }}</v-card-title>
        <v-card-text class="text-medium-emphasis px-0 pb-6">
          <v-text-field
            v-model="renameName"
            :label="$t('devices.device_name')"
            variant="outlined"
            density="comfortable"
            hide-details="auto"
            @keyup.enter="confirmRename"
          ></v-text-field>
        </v-card-text>
        <v-card-actions class="px-0 ga-3">
          <v-btn
            variant="text"
            class="flex-grow-1 text-high-emphasis"
            height="44"
            @click="renameDialog = false"
          >
            {{ $t('common.cancel') }}
          </v-btn>
          <v-btn
            variant="flat"
            color="primary"
            class="flex-grow-1"
            height="44"
            @click="confirmRename"
            :loading="renaming"
          >
            {{ $t('common.save') }}
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-container>
</template>

<style scoped>
.device-icon-wrap {
  position: relative;
}
.device-status-dot {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: 2px solid #fff;
}
.dot-connected {
  background: rgb(var(--v-theme-success));
}
.dot-offline {
  background: rgb(var(--v-theme-error));
}
.dot-idle {
  background: rgba(var(--v-theme-on-surface), 0.7);
}
.device-card {
  transition: all 0.2s ease;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12) !important;
}

.device-card:hover {
  background: color-mix(in srgb, rgb(var(--v-theme-on-surface)) 6%, transparent) !important;
  transform: translateY(-2px);
  border-color: rgba(var(--v-theme-on-surface), 0.12) !important;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05) !important;
}

.animate-fade-in {
  animation: fadeIn 0.4s ease-out;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.max-w-400 {
  max-width: 400px;
}
.uppercase {
  text-transform: uppercase;
}
.tracking-wider {
  letter-spacing: 0.05em;
}
.capitalize {
  text-transform: capitalize;
}
</style>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import ConnectView from './ConnectView.vue';
import PageLoading from './shared/PageLoading.vue';
import { useSyncStore } from '@/stores/sync';
import { deviceOsIcon } from '@/utils/format';
import {
  listDevices,
  removeDevice as removeDeviceApi,
  renameDevice as renameDeviceApi,
  requestStartSync,
} from '@/services/tauri';

interface DeviceWithSync {
  id: string;
  title: string;
  icon: string;
  os: string;
  photo_count: number;
  video_count: number;
  remote_photo_count: number;
  remote_video_count: number;
  host: string;
  subtitle: string;
  syncing: boolean;
  progress: number;
  syncStatus: string;
  items_completed: number;
  items_total: number;
}

const devices = ref<DeviceWithSync[]>([]);
const loading = ref(true);
const deleteDialog = ref(false);
const deviceToDelete = ref('');
const deleting = ref(false);

const renameDialog = ref(false);
const renameId = ref('');
const renameName = ref('');
const renaming = ref(false);
const reconnecting = ref(false);

const syncStore = useSyncStore();
const { t } = useI18n();
const connection = computed(() => syncStore.connection);

const dotClass = computed(() => {
  if (syncStore.connection === 'connected') return 'dot-connected';
  if (syncStore.connection === 'offline') return 'dot-offline';
  return 'dot-idle';
});

const dotTitle = computed(() => {
  if (syncStore.connection === 'connected') return t('devices.status_connected');
  if (syncStore.connection === 'offline') return t('devices.status_offline');
  return t('devices.status_idle');
});

let unlistenRefresh: UnlistenFn | null = null;
let unlistenSync: UnlistenFn | null = null;

async function loadDevices() {
  loading.value = true;
  try {
    const realDevices = await listDevices();
    devices.value = (realDevices || []).map((d) => ({
      ...d,
      syncing: false,
      progress: 0,
      syncStatus: '',
      items_completed: 0,
      items_total: 0,
    }));
  } catch (err) {
    console.error('Failed to list devices:', err);
  } finally {
    loading.value = false;
  }
}

async function startSync() {
  try {
    await requestStartSync();
  } catch (err) {
    console.error('Failed to request sync:', err);
  }
}

async function reconnect() {
  reconnecting.value = true;
  try {
    await syncStore.reconnect();
    await loadDevices();
  } finally {
    reconnecting.value = false;
  }
}

function openRename(device: DeviceWithSync) {
  renameId.value = device.id;
  renameName.value = device.title;
  renameDialog.value = true;
}

async function confirmRename() {
  const name = renameName.value.trim();
  if (!name) return;
  renaming.value = true;
  try {
    await renameDeviceApi(renameId.value, name);
    await loadDevices();
    renameDialog.value = false;
  } catch (err) {
    console.error('Failed to rename device:', err);
  } finally {
    renaming.value = false;
  }
}

function removeDevice(id: string) {
  deviceToDelete.value = id;
  deleteDialog.value = true;
}

async function confirmDelete() {
  deleting.value = true;
  try {
    await removeDeviceApi(deviceToDelete.value);
    await loadDevices();
    deleteDialog.value = false;
  } catch (err) {
    console.error('Failed to remove device:', err);
  } finally {
    deleting.value = false;
    deviceToDelete.value = '';
  }
}

onMounted(async () => {
  await loadDevices();

  unlistenRefresh = await listen('refresh-devices', () => {
    loadDevices();
  });

  unlistenSync = await listen('sync-progress', (event) => {
    const payload = event.payload as {
      status: string;
      progress: number;
      items_completed: number;
      items_total: number;
    };
    devices.value.forEach((d) => {
      if (!d.host) {
        d.syncing =
          payload.status !== 'idle' &&
          !payload.status.includes('Finished') &&
          !payload.status.includes('Up to date') &&
          !payload.status.includes('All files synced');
        d.items_completed = payload.items_completed;
        d.items_total = payload.items_total;
        // The bar tracks overall batch progress, never per-file bytes.
        d.progress =
          payload.items_total > 0
            ? (payload.items_completed / payload.items_total) * 100
            : d.progress;
        d.syncStatus = payload.status;
      }
    });
  });
});

onUnmounted(() => {
  if (unlistenRefresh) unlistenRefresh();
  if (unlistenSync) unlistenSync();
});
</script>
