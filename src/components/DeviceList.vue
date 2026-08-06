<template>
  <v-container class="pa-6 bg-siegu-main">
    <div class="d-flex align-center justify-space-between mb-8">
      <div>
        <div class="d-flex align-center mb-1">
          <v-icon color="var(--color-text-primary)" size="28" class="mr-3">mdi-devices</v-icon>
          <h1 class="text-h4 font-weight-bold text-zinc-primary">{{ $t('devices.title') }}</h1>
        </div>
        <div class="text-subtitle-1 text-zinc-secondary">{{ $t('devices.desc') }}</div>
      </div>
      <ConnectView />
    </div>

    <!-- Empty State -->
    <div
      v-if="devices.length === 0"
      class="d-flex flex-column align-center justify-center py-16 text-center animate-fade-in"
    >
      <v-icon size="64" color="var(--color-icon-empty)" class="mb-4">mdi-laptop-off</v-icon>
      <div class="text-h6 text-zinc-secondary font-weight-bold">{{ $t('devices.no_devices') }}</div>
      <p class="text-body-2 text-zinc-muted mt-1 max-w-400 mx-auto">
        {{ $t('devices.no_devices_desc') }}
      </p>
    </div>

    <v-row v-else>
      <v-col cols="12" sm="6" md="4" v-for="device in devices" :key="device.title">
        <v-card
          variant="flat"
          height="100%"
          class="device-card border-subtle ga-2"
          rounded="xl"
          color="surface"
        >
          <v-card-item class="py-4">
            <template v-slot:prepend>
              <div class="siegu-icon-circle-dark mr-3 device-icon-wrap">
                <v-icon color="var(--color-text-btn)" size="small">{{ device.icon }}</v-icon>
                <span
                  v-if="!device.host"
                  class="device-status-dot"
                  :class="dotClass"
                  :title="dotTitle"
                ></span>
              </div>
            </template>
            <v-card-title
              class="text-zinc-primary text-subtitle-1 font-weight-bold d-flex align-center"
            >
              {{ device.title }}
              <v-chip
                v-if="device.host"
                size="x-small"
                variant="flat"
                color="black"
                class="text-white ml-2 font-weight-bold"
                style="height: 18px"
                >{{ $t('devices.this_device') }}</v-chip
              >
            </v-card-title>
            <v-card-subtitle class="text-zinc-secondary text-caption">{{
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
                      class="text-zinc-muted"
                    ></v-btn>
                  </template>
                  <v-list density="compact" rounded="lg" class="border-subtle">
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
                      <v-list-item-title class="text-error font-weight-bold">{{
                        $t('devices.remove_device')
                      }}</v-list-item-title>
                    </v-list-item>
                  </v-list>
                </v-menu>
              </div>
            </template>
          </v-card-item>

          <v-card-text class="pt-0">
            <!-- Device Details -->
            <div class="bg-zinc-50 rounded-lg pa-3 mb-4 border-subtle">
              <div class="d-flex align-center mb-2">
                <v-icon size="14" color="zinc-muted" class="mr-2">mdi-desktop-tower-monitor</v-icon>
                <span
                  class="text-caption text-zinc-secondary font-weight-bold uppercase tracking-wider"
                  >{{ $t('devices.system') }}</span
                >
                <v-spacer></v-spacer>
                <span class="text-caption text-zinc-primary font-weight-bold capitalize">{{
                  device.os
                }}</span>
              </div>
              <div class="d-flex align-center mb-2">
                <v-icon size="14" color="zinc-muted" class="mr-2"
                  >mdi-image-multiple-outline</v-icon
                >
                <span class="text-caption text-zinc-secondary">{{ $t('media.label_photos') }}</span>
                <v-spacer></v-spacer>
                <span class="text-caption text-zinc-primary font-weight-bold">{{
                  device.photo_count
                }}</span>
              </div>
              <div class="d-flex align-center">
                <v-icon size="14" color="zinc-muted" class="mr-2">mdi-video-outline</v-icon>
                <span class="text-caption text-zinc-secondary">{{ $t('media.label_videos') }}</span>
                <v-spacer></v-spacer>
                <span class="text-caption text-zinc-primary font-weight-bold">{{
                  device.video_count
                }}</span>
              </div>
            </div>

            <div v-if="device.syncing" class="mt-4">
              <div class="d-flex align-center justify-space-between mb-1">
                <span class="text-caption text-zinc-muted text-truncate mr-2">{{
                  device.syncStatus
                }}</span>
                <span
                  class="text-caption text-zinc-primary font-weight-bold"
                  v-if="device.items_total > 0"
                >
                  {{ device.items_completed }}/{{ device.items_total }}
                </span>
              </div>
              <v-progress-linear
                :model-value="device.progress"
                color="black"
                height="6"
                rounded
                bg-color="var(--color-bg-zinc-100)"
                bg-opacity="1"
              ></v-progress-linear>
            </div>
            <div v-else class="d-flex align-center mt-2">
              <v-btn
                v-if="!device.host && connection === 'connected'"
                variant="flat"
                color="black"
                class="siegu-btn flex-grow-1"
                size="small"
                @click="startSync"
              >
                <v-icon start size="small">mdi-sync</v-icon>
                {{ $t('devices.sync_now') }}
              </v-btn>
              <v-btn
                v-else-if="!device.host && connection === 'offline'"
                variant="tonal"
                color="black"
                class="siegu-btn flex-grow-1"
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
                class="text-white text-none border-subtle"
              >
                {{ $t('devices.online') }}
              </v-chip>
            </div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>

    <!-- Delete Confirmation Dialog -->
    <v-dialog v-model="deleteDialog" max-width="400" rounded="xl">
      <v-card class="pa-6 border-subtle bg-siegu-white">
        <div class="siegu-icon-circle-error mb-4">
          <v-icon color="white">mdi-alert-outline</v-icon>
        </div>
        <v-card-title class="text-h5 font-weight-bold text-zinc-primary px-0 pb-2">{{
          $t('devices.remove_device_title')
        }}</v-card-title>
        <v-card-text class="text-zinc-secondary px-0 pb-6">
          <span>{{ $t('devices.remove_device_confirm', { name: deviceToDelete }) }}</span>
        </v-card-text>
        <v-card-actions class="px-0 ga-3">
          <v-btn
            variant="flat"
            color="var(--color-bg-zinc-100)"
            class="siegu-btn flex-grow-1 text-zinc-primary"
            height="44"
            @click="deleteDialog = false"
          >
            {{ $t('common.cancel') }}
          </v-btn>
          <v-btn
            variant="flat"
            color="error"
            class="siegu-btn flex-grow-1"
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
      <v-card class="pa-6 border-subtle bg-siegu-white">
        <div class="siegu-icon-circle-dark mb-4">
          <v-icon color="var(--color-text-btn)">mdi-pencil-outline</v-icon>
        </div>
        <v-card-title class="text-h5 font-weight-bold text-zinc-primary px-0 pb-2">{{
          $t('devices.rename_device_title')
        }}</v-card-title>
        <v-card-text class="text-zinc-secondary px-0 pb-6">
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
            variant="flat"
            color="var(--color-bg-zinc-100)"
            class="siegu-btn flex-grow-1 text-zinc-primary"
            height="44"
            @click="renameDialog = false"
          >
            {{ $t('common.cancel') }}
          </v-btn>
          <v-btn
            variant="flat"
            color="black"
            class="siegu-btn flex-grow-1"
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
.siegu-icon-circle-error {
  width: 48px;
  height: 48px;
  background: var(--color-error);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: color-mix(in srgb, var(--color-error) 20%, transparent) 0 4px 12px;
}
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
  background: var(--color-success);
}
.dot-offline {
  background: var(--color-error);
}
.dot-idle {
  background: var(--color-text-secondary);
}
.device-card {
  transition: all 0.2s ease;
  border: 1px solid var(--color-border-subtle) !important;
}

.device-card:hover {
  background: var(--color-bg-hover) !important;
  transform: translateY(-2px);
  border-color: var(--color-border-default) !important;
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
import { useSyncStore } from '@/stores/sync';
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
  host: string;
  subtitle: string;
  syncing: boolean;
  progress: number;
  syncStatus: string;
  items_completed: number;
  items_total: number;
}

const devices = ref<DeviceWithSync[]>([]);
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
          !payload.status.includes('Up to date');
        d.progress = payload.progress;
        d.syncStatus = payload.status;
        d.items_completed = payload.items_completed;
        d.items_total = payload.items_total;
      }
    });
  });
});

onUnmounted(() => {
  if (unlistenRefresh) unlistenRefresh();
  if (unlistenSync) unlistenSync();
});
</script>
