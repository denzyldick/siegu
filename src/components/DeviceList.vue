<template>
  <v-container class="pa-6 bg-siegu-main">
    <div class="d-flex align-center justify-space-between mb-8">
      <div>
        <div class="d-flex align-center mb-1">
          <v-icon color="#18181b" size="28" class="mr-3">mdi-devices</v-icon>
          <h1 class="text-h4 font-weight-bold text-zinc-primary">{{ $t('devices.title') }}</h1>
        </div>
        <div class="text-subtitle-1 text-zinc-secondary">{{ $t('devices.desc') }}</div>
      </div>
      <Connect />
    </div>

    <!-- Empty State -->
    <div v-if="devices.length === 0" class="d-flex flex-column align-center justify-center py-16 text-center animate-fade-in">
      <v-icon size="64" color="#d4d4d8" class="mb-4">mdi-laptop-off</v-icon>
      <div class="text-h6 text-zinc-secondary font-weight-bold">{{ $t('devices.no_devices') }}</div>
      <p class="text-body-2 text-zinc-muted mt-1 max-w-400 mx-auto">{{ $t('devices.no_devices_desc') }}</p>
    </div>

    <v-row v-else>
      <v-col cols="12" sm="6" md="4" v-for="device in devices" :key="device.title">
        <v-card variant="flat" height="100%" class="device-card border-subtle ga-2" rounded="xl" color="#ffffff">
          <v-card-item class="py-4">
            <template v-slot:prepend>
              <div class="siegu-icon-circle-dark mr-3">
                <v-icon color="#ffffff" size="small">{{ device.icon }}</v-icon>
              </div>
            </template>
            <v-card-title class="text-zinc-primary text-subtitle-1 font-weight-bold d-flex align-center">
              {{ device.title }}
              <v-chip v-if="device.host" size="x-small" variant="flat" color="black" class="text-white ml-2 font-weight-bold" style="height: 18px">{{ $t('devices.this_device') }}</v-chip>
            </v-card-title>
            <v-card-subtitle class="text-zinc-secondary text-caption">{{ device.host ? $t('devices.local_environment') : (device.subtitle || $t('devices.connected')) }}</v-card-subtitle>

            <template v-slot:append>
               <div class="d-flex align-center ga-1">
                <v-menu v-if="!device.host">
                  <template v-slot:activator="{ props }">
                    <v-btn icon="mdi-dots-vertical" variant="text" size="small" v-bind="props" class="text-zinc-muted"></v-btn>
                  </template>
                  <v-list density="compact" rounded="lg" class="border-subtle">
                    <v-list-item @click="removeDevice(device.title)" color="error">
                      <template v-slot:prepend>
                        <v-icon size="small" color="error">mdi-delete-outline</v-icon>
                      </template>
                      <v-list-item-title class="text-error font-weight-bold">{{ $t('devices.remove_device') }}</v-list-item-title>
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
                    <span class="text-caption text-zinc-secondary font-weight-bold uppercase tracking-wider">{{ $t('devices.system') }}</span>
                    <v-spacer></v-spacer>
                    <span class="text-caption text-zinc-primary font-weight-bold capitalize">{{ device.os }}</span>
                  </div>
                  <div class="d-flex align-center mb-2">
                    <v-icon size="14" color="zinc-muted" class="mr-2">mdi-image-multiple-outline</v-icon>
                    <span class="text-caption text-zinc-secondary">{{ $t('photos.label_photos') }}</span>
                    <v-spacer></v-spacer>
                    <span class="text-caption text-zinc-primary font-weight-bold">{{ device.photo_count }}</span>
                  </div>
                  <div class="d-flex align-center">
                    <v-icon size="14" color="zinc-muted" class="mr-2">mdi-video-outline</v-icon>
                    <span class="text-caption text-zinc-secondary">{{ $t('photos.label_videos') }}</span>
                    <v-spacer></v-spacer>
                    <span class="text-caption text-zinc-primary font-weight-bold">{{ device.video_count }}</span>
                  </div>
               </div>

               <div v-if="device.syncing" class="mt-4">
                   <div class="d-flex align-center justify-space-between mb-1">
                       <span class="text-caption text-zinc-muted text-truncate mr-2">{{ device.syncStatus }}</span>
                       <span class="text-caption text-zinc-primary font-weight-bold" v-if="device.items_total > 0">
                         {{ device.items_completed }}/{{ device.items_total }}
                       </span>
                   </div>
                   <v-progress-linear
                     :model-value="device.progress"
                     color="black"
                     height="6"
                     rounded
                     bg-color="#f4f4f5"
                     bg-opacity="1"
                   ></v-progress-linear>
               </div>
               <div v-else class="d-flex align-center mt-2">
                   <v-btn v-if="!device.host" variant="flat" color="black" class="siegu-btn flex-grow-1" size="small" @click="startSync">
                    <v-icon start size="small">mdi-sync</v-icon>
                    {{ $t('devices.sync_now') }}
                   </v-btn>
                    <v-chip v-else size="x-small" color="success" variant="flat" class="text-white text-none border-subtle">
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
        <v-card-title class="text-h5 font-weight-bold text-zinc-primary px-0 pb-2">{{ $t('devices.remove_device_title') }}</v-card-title>
        <v-card-text class="text-zinc-secondary px-0 pb-6">
          <span v-html="$t('devices.remove_device_confirm', { name: deviceToDelete })"></span>
        </v-card-text>
        <v-card-actions class="px-0 ga-3">
          <v-btn variant="flat" color="#f4f4f5" class="siegu-btn flex-grow-1 text-zinc-primary" height="44" @click="deleteDialog = false">
            {{ $t('common.cancel') }}
          </v-btn>
          <v-btn variant="flat" color="error" class="siegu-btn flex-grow-1" height="44" @click="confirmDelete" :loading="deleting">
            {{ $t('common.remove') }}
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
  background: #ef4444;
  border-radius: 50%;
  display: flex;
  align-center: center;
  justify-content: center;
  box-shadow: 0 4px 12px rgba(239, 68, 68, 0.2);
}
.device-card {
  transition: all 0.2s ease;
  border: 1px solid rgba(0, 0, 0, 0.05) !important;
}

.device-card:hover {
  background: #ffffff !important;
  transform: translateY(-2px);
  border-color: rgba(0, 0, 0, 0.1) !important;
  box-shadow: 0 4px 12px rgba(0,0,0,0.05) !important;
}

.animate-fade-in {
  animation: fadeIn 0.4s ease-out;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
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

<script>
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Connect from "./Connect.vue";

export default {
  name: "DeviceList",
  components: {
    Connect,
  },
  data: () => ({
      devices: [],
      syncStates: {},
      deleteDialog: false,
      deviceToDelete: "",
      deleting: false,
  }),
  async mounted() {
      await this.list_devices();

      listen("refresh-devices", () => {
          this.list_devices();
      });

      listen("sync-progress", (event) => {
          const payload = event.payload;
          // We assume 'peer' refers to any connected device for now 
          // (multi-device support would need device_id mapping)
          this.devices.forEach(d => {
            if (!d.host) {
              d.syncing = payload.status !== 'idle' && !payload.status.includes('Finished') && !payload.status.includes('Up to date');
              d.progress = payload.progress;
              d.syncStatus = payload.status;
              d.items_completed = payload.items_completed;
              d.items_total = payload.items_total;
            }
          });
      });
  },
  methods: {
    async list_devices() {
      try {
        const realDevicesStr = await invoke("list_devices");
        const realDevices = JSON.parse(realDevicesStr);

        this.devices = (realDevices || []).map(d => ({
            ...d,
            syncing: false,
            progress: 0,
            syncStatus: '',
            items_completed: 0,
            items_total: 0
        }));
      } catch (err) {
        console.error("Failed to list devices:", err);
      }
    },
    async startSync() {
      try {
        await invoke("request_start_sync");
      } catch (err) {
        console.error("Failed to request sync:", err);
      }
    },
    removeDevice(name) {
      this.deviceToDelete = name;
      this.deleteDialog = true;
    },
    async confirmDelete() {
      this.deleting = true;
      try {
        await invoke("remove_device", { name: this.deviceToDelete });
        await this.list_devices();
        this.deleteDialog = false;
      } catch (err) {
        console.error("Failed to remove device:", err);
      } finally {
        this.deleting = false;
        this.deviceToDelete = "";
      }
    }
  },
};
</script>
