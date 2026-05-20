<template>
  <div class="connect-wrapper">
    <v-dialog v-model="dialog" width="auto" scrim="black" transition="dialog-bottom-transition">
      <template v-slot:activator="{ props }">
        <v-btn
          v-if="!embedded"
          v-bind="props"
          color="#000000"
          theme="dark"
          variant="flat"
          class="siegu-btn px-6"
          height="44"
        >
          <div class="d-flex align-center">
            <div class="siegu-icon-circle siegu-icon-circle-md mr-3">
              <v-icon size="14" color="white">mdi-plus</v-icon>
            </div>
            <span class="text-white font-weight-bold">{{ $t('devices.add_device') }}</span>
          </div>
        </v-btn>
      </template>

      <v-card v-if="!embedded" class="border-subtle pa-6 text-center bg-siegu-white" rounded="xl" min-width="350" max-width="400">
        <div class="text-h5 font-weight-bold text-zinc-primary mb-2">{{ $t('connect.link_device_title') }}</div>
        <div class="text-body-2 text-zinc-secondary mb-6">
          {{ $t('connect.link_device_desc') }}
        </div>

        <div class="d-flex justify-center mb-6" v-if="!hideModeToggle">
            <v-btn-toggle v-model="mode" mandatory variant="flat" class="ga-2 bg-transparent">
              <v-btn value="host" class="siegu-btn text-none px-6">{{ $t('connect.host') }}</v-btn>
              <v-btn value="join" class="siegu-btn text-none px-6">{{ $t('connect.join') }}</v-btn>
            </v-btn-toggle>
        </div>

        <div class="d-flex justify-center mb-6" v-if="uuid && mode === 'host'">
          <v-sheet class="bg-siegu-white rounded-xl pa-6 shadow-lg border-subtle position-relative overflow-hidden">
                <v-fade-transition hide-on-leave>
                  <div v-if="uuid && peerJoined && !isConnected" class="overlay-connecting d-flex flex-column align-center justify-center">
                     <v-progress-circular indeterminate color="black" size="48" width="4" class="mb-4"></v-progress-circular>
                     <div class="text-subtitle-2 font-weight-bold text-zinc-primary animate-pulse">{{ $t('connect.device_found') }}</div>
                     <div class="text-caption text-zinc-secondary">{{ $t('connect.establishing_link') }}</div>
                  </div>
                </v-fade-transition>
                <v-fade-transition hide-on-leave>
                  <div v-if="isConnected" class="overlay-connecting d-flex flex-column align-center justify-center bg-white">
                     <div class="siegu-icon-circle-success mx-auto mb-4 scale-up">
                        <v-icon color="white">mdi-check-bold</v-icon>
                     </div>
                     <div class="text-subtitle-2 font-weight-bold text-zinc-primary">{{ $t('connect.link_established') }}</div>
                     <div class="text-caption text-zinc-secondary">{{ $t('connect.ready_to_sync') }}</div>
                  </div>
                </v-fade-transition>
                <qrcode-vue :value="uuid" :size="200" level="H" :class="{'opacity-20 blur-sm transition-all': uuid && (peerJoined || isConnected)}" />
          </v-sheet>
        </div>

        <div class="text-caption text-zinc-muted mb-2 uppercase tracking-widest" v-if="mode === 'host'">{{ $t('connect.manual_phrase') }}</div>
        <div class="d-flex justify-center flex-wrap gap-2 mb-6" v-if="passphrase.length > 0 && mode === 'host'">
          <v-chip
            v-for="(word, index) in passphrase"
            :key="index"
            color="#f4f4f5"
            variant="flat"
            class="font-weight-medium mx-1 text-zinc-primary border-subtle"
            size="small"
          >
            {{ word }}
          </v-chip>
        </div>

        <div class="d-flex justify-center mb-6 flex-column ga-4" v-if="mode === 'join'">
            <v-card variant="flat" class="bg-zinc-50 border-subtle pa-6 rounded-xl mb-2 text-center w-100">
              <div v-if="!isScanning">
                <div class="siegu-icon-circle mx-auto mb-4">
                  <v-icon color="white">mdi-qrcode-scan</v-icon>
                </div>
                <div class="text-h6 font-weight-bold text-zinc-primary mb-2">{{ $t('connect.scan_qr') }}</div>
                <p class="text-caption text-zinc-secondary mb-6">{{ $t('connect.scan_qr_desc') }}</p>
                <v-btn color="black" block height="56" class="siegu-btn" @click="startScanner">
                  {{ $t('connect.open_camera') }}
                </v-btn>
              </div>
              
              <div v-else class="position-relative">
                <video ref="scannerVideo" style="width: 100%; border-radius: 12px; background: black; max-height: 250px; object-fit: cover;"></video>
                <v-btn icon size="small" color="white" class="position-absolute" @click="stopScanner" style="position: absolute; top: 8px; right: 8px; z-index: 20;">
                  <v-icon>mdi-close</v-icon>
                </v-btn>
                <div class="scanner-overlay"></div>
              </div>
            </v-card>

            <div class="text-caption text-zinc-muted text-center uppercase tracking-widest">{{ $t('connect.or_enter_manually') }}</div>

            <v-text-field
              v-model="joinPassphrase"
              :placeholder="$t('connect.phrase_placeholder')"
              variant="solo-filled"
              density="comfortable"
              hide-details
              flat
              rounded="lg"
              class="text-center siegu-field"
              @keyup.enter="joinWebRTC"
              :disabled="loading || isConnected"
            ></v-text-field>
            
            <v-btn v-if="!isConnected" variant="flat" @click="joinWebRTC" class="siegu-btn py-6" block :loading="loading" :disabled="!joinPassphrase">
              <div class="d-flex align-center">
                <div class="siegu-icon-circle mr-3">
                  <v-icon size="14" color="white">mdi-link-variant</v-icon>
                </div>
                <span class="text-white">{{ $t('connect.link_device_button') }}</span>
              </div>
            </v-btn>

            <v-btn v-else variant="flat" color="success" @click="triggerSync" class="siegu-btn py-6" block :loading="syncing">
              <div class="d-flex align-center">
                <div class="siegu-icon-circle mr-3">
                  <v-icon size="14" color="white">mdi-sync</v-icon>
                </div>
                <span class="text-white">{{ $t('connect.start_syncing') }}</span>
              </div>
            </v-btn>
        </div>

        <div class="d-flex justify-center mb-6 flex-column ga-4" v-if="mode === 'host' && isConnected">
            <v-btn variant="flat" color="success" @click="triggerSync" class="siegu-btn py-6" block :loading="syncing">
              <div class="d-flex align-center">
                <div class="siegu-icon-circle mr-3">
                  <v-icon size="14" color="white">mdi-sync</v-icon>
                </div>
                <span class="text-white">{{ $t('connect.start_syncing') }}</span>
              </div>
            </v-btn>
        </div>

        <div class="text-caption text-zinc-muted mb-1 text-center py-2" v-if="connectionStatus">
            <v-progress-circular v-if="!isConnected && connectionStatus !== $t('connect.disconnected')" indeterminate color="black" size="16" width="2" class="mr-2 opacity-50"></v-progress-circular>
            <v-icon v-else-if="isConnected" color="success" size="16" class="mr-2">mdi-check-circle-outline</v-icon>
            {{ connectionStatus }}
        </div>

        <div v-if="syncProgress.status" class="mt-4 px-4">
          <div class="d-flex justify-space-between text-caption text-zinc-secondary mb-1">
            <span>{{ syncProgress.status }}</span>
            <span v-if="syncProgress.progress > 0">{{ Math.round(syncProgress.progress) }}%</span>
          </div>
          <v-progress-linear
            v-model="syncProgress.progress"
            color="black"
            height="6"
            rounded
            indeterminate
            v-if="syncProgress.progress === 0 && syncProgress.status.includes('Syncing')"
          ></v-progress-linear>
          <v-progress-linear
            v-else
            v-model="syncProgress.progress"
            color="black"
            height="6"
            rounded
          ></v-progress-linear>
        </div>

        <v-btn v-if="isConnected || (connectionStatus && connectionStatus !== 'Disconnected')" 
               variant="text" color="error" size="small" class="mt-4 text-none" 
               @click="disconnectSession" :loading="disconnecting">
          {{ $t('devices.disconnect') }}
        </v-btn>

        <v-divider class="opacity-10 my-4"></v-divider>

        <v-card-actions class="justify-center">
          <v-btn variant="flat" color="#18181b" class="siegu-btn px-6" @click="dialog = false">
              <div class="d-flex align-center">
                <div class="siegu-icon-circle siegu-icon-circle-sm mr-2">
                  <v-icon size="12" color="white">mdi-close</v-icon>
                </div>
                <span class="text-white font-weight-bold">{{ $t('common.close') }}</span>
              </div>
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Inline version for onboarding -->
    <div v-if="embedded" class="w-100">
      <div class="d-flex justify-center mb-6" v-if="!hideModeToggle">
          <v-btn-toggle v-model="mode" mandatory variant="flat" class="ga-2 bg-transparent">
            <v-btn value="host" class="siegu-btn text-none px-6">{{ $t('connect.host') }}</v-btn>
              <v-btn value="join" class="siegu-btn text-none px-6">{{ $t('connect.join') }}</v-btn>
            </v-btn-toggle>
        </div>

        <div class="d-flex justify-center mb-6" v-if="uuid && mode === 'host'">
          <v-sheet class="bg-siegu-white rounded-xl pa-6 shadow-lg border-subtle position-relative overflow-hidden">
               <v-fade-transition hide-on-leave>
                 <div v-if="uuid && (connectionStatus === 'Peer Joined' || connectionStatus === 'Connected' || connectionStatus === 'connected')" class="overlay-connecting d-flex flex-column align-center justify-center">
                    <v-progress-circular indeterminate color="black" size="48" width="4" class="mb-4"></v-progress-circular>
                    <div class="text-subtitle-2 font-weight-bold text-zinc-primary animate-pulse">{{ $t('connect.device_found') }}</div>
                    <div class="text-caption text-zinc-secondary">{{ $t('connect.establishing_link') }}</div>
                </div>
              </v-fade-transition>
              <qrcode-vue :value="uuid" :size="200" level="H" :class="{'opacity-20 blur-sm transition-all': uuid && (connectionStatus === 'Peer Joined' || connectionStatus === 'Connected' || connectionStatus === 'connected')}" />
        </v-sheet>
      </div>

      <div class="text-caption text-zinc-muted mb-2 uppercase tracking-widest text-center" v-if="mode === 'host'">{{ $t('connect.manual_phrase') }}</div>
      <div class="d-flex justify-center flex-wrap gap-2 mb-6" v-if="passphrase.length > 0 && mode === 'host'">
        <v-chip
          v-for="(word, index) in passphrase"
          :key="index"
          color="#f4f4f5"
          variant="flat"
          class="font-weight-medium mx-1 text-zinc-primary border-subtle"
          size="small"
        >
          {{ word }}
        </v-chip>
      </div>

      <div class="d-flex justify-center mb-6 flex-column ga-4" v-if="mode === 'join'">
          <v-card variant="flat" class="bg-zinc-50 border-subtle pa-6 rounded-xl mb-2 text-center w-100">
            <div v-if="!isScanning">
              <div class="siegu-icon-circle mx-auto mb-4">
                <v-icon color="white">mdi-qrcode-scan</v-icon>
              </div>
              <div class="text-h6 font-weight-bold text-zinc-primary mb-2">{{ $t('connect.scan_qr') }}</div>
              <p class="text-caption text-zinc-secondary mb-6">{{ $t('connect.scan_qr_desc') }}</p>
              <v-btn color="black" block height="56" class="siegu-btn" @click="startScanner">
                {{ $t('connect.open_camera') }}
              </v-btn>
            </div>
            
            <div v-else class="position-relative">
              <video ref="scannerVideo" style="width: 100%; border-radius: 12px; background: black; max-height: 250px; object-fit: cover;"></video>
              <v-btn icon size="small" color="white" class="position-absolute" @click="stopScanner" style="position: absolute; top: 8px; right: 8px; z-index: 20;">
                <v-icon>mdi-close</v-icon>
              </v-btn>
              <div class="scanner-overlay"></div>
            </div>
          </v-card>

          <div class="text-caption text-zinc-muted text-center uppercase tracking-widest">{{ $t('connect.or_enter_manually') }}</div>

          <v-text-field
            v-model="joinPassphrase"
            :placeholder="$t('connect.phrase_placeholder')"
            variant="solo-filled"
            density="comfortable"
            hide-details
            flat
            rounded="lg"
            class="text-center siegu-field"
            @keyup.enter="joinWebRTC"
            :disabled="loading || isConnected"
          ></v-text-field>
          <v-btn variant="flat" @click="joinWebRTC" class="siegu-btn py-6" block :loading="loading" :disabled="!joinPassphrase || isConnected">
            <div class="d-flex align-center">
              <div class="siegu-icon-circle mr-3">
                <v-icon size="14" color="white">mdi-link-variant</v-icon>
              </div>
              <span class="text-white">{{ $t('connect.link_device_button') }}</span>
            </div>
          </v-btn>
      </div>

      <div class="text-caption text-zinc-muted mb-1 text-center py-2" v-if="connectionStatus">
          <v-progress-circular v-if="!isConnected" indeterminate color="black" size="16" width="2" class="mr-2 opacity-50"></v-progress-circular>
          <v-icon v-else color="success" size="16" class="mr-2">mdi-check-circle-outline</v-icon>
          {{ connectionStatus }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.uppercase {
  text-transform: uppercase;
}
.tracking-widest {
  letter-spacing: 0.1em;
}

.overlay-connecting {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 10;
  background: rgba(255, 255, 255, 0.7);
  backdrop-filter: blur(4px);
}

.blur-sm {
  filter: blur(2px);
}

.opacity-20 {
  opacity: 0.2;
}

.transition-all {
  transition: all 0.3s ease;
}

.animate-pulse {
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}

.position-relative {
  position: relative;
}

.overflow-hidden {
  overflow: hidden;
}

.siegu-icon-circle {
  width: 28px;
  height: 28px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.siegu-icon-circle-success {
  width: 48px;
  height: 48px;
  background: #22c55e;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 4px 12px rgba(34, 197, 94, 0.3);
}

.scale-up {
  animation: scaleUp 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

@keyframes scaleUp {
  from { transform: scale(0.5); opacity: 0; }
  to { transform: scale(1); opacity: 1; }
}
</style>

<script>
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import QrcodeVue from 'qrcode.vue';
import { BrowserQRCodeReader } from '@zxing/library';

export default {
  name: "Connect",
  emits: ["connected", "mode-change"],
  components: {
    QrcodeVue,
  },
  props: {
    embedded: { type: Boolean, default: false },
    initialMode: { type: String, default: 'host' },
    hideModeToggle: { type: Boolean, default: false }
  },
  data: () => ({
    dialog: false,
    mode: "host",
    uuid: "",
    passphrase: [],
    joinPassphrase: "",
    connectionStatus: "",
    isConnected: false,
    peerJoined: false,
    loading: false,
    syncing: false,
    disconnecting: false,
    unlisten: null,
    unlistenSync: null,
    syncProgress: {
      status: "",
      progress: 0
    },
    codeReader: new BrowserQRCodeReader(),
    isScanning: false,
  }),
  watch: {
    mode(newMode) {
       this.$emit('mode-change', newMode);
       if (newMode === 'host' && !this.uuid) {
           this.initialize();
       } else if (newMode === 'join') {
           this.connectionStatus = "";
           this.peerJoined = false;
           this.syncProgress = { status: "", progress: 0 };
       }
    },
    async dialog(val) {
      if (val) {
        this.unlisten = await listen("webrtc-state", (event) => {
          this.connectionStatus = event.payload;
          if (event.payload === "Peer Joined") {
            this.peerJoined = true;
          }
          if (event.payload === "Connected" || event.payload === "connected") {
            this.isConnected = true;
            this.peerJoined = false;
            this.loading = false;
            this.$emit('connected');
          }
          if (event.payload.toLowerCase().includes("error") ||
              event.payload.toLowerCase().includes("failed") ||
              event.payload.toLowerCase().includes("disconnected")) {
            this.isConnected = false;
            this.peerJoined = false;
            this.loading = false;
          }
        });
        this.unlistenSync = await listen("sync-progress", (event) => {
          this.syncProgress = {
            status: event.payload.status,
            progress: event.payload.progress
          };

          if (event.payload.status.toLowerCase().includes("syncing")) {
             this.syncing = true;
          }

          if (event.payload.status === "Up to date" || event.payload.status.startsWith("Finished")) {
             this.syncing = false;
             setTimeout(() => {
               if (this.syncProgress.status === event.payload.status) {
                  this.syncProgress = { status: "", progress: 0 };
                  if (this.dialog) {
                    this.dialog = false;
                    this.$emit('done');
                  }
               }
             }, 2000);
          }
        });
        this.initialize();
      } else {
        if (this.unlisten) {
          this.unlisten();
          this.unlisten = null;
        }
        if (this.unlistenSync) {
          this.unlistenSync();
          this.unlistenSync = null;
        }
        this.loading = false;
      }
    }
  },
  async mounted() {
    this.mode = this.initialMode;
    if (this.embedded) {
      this.unlisten = await listen("webrtc-state", (event) => {
        this.connectionStatus = event.payload;
        if (event.payload === "Connected" || event.payload === "connected") {
          this.isConnected = true;
          this.loading = false;
          this.$emit('connected');
        }
      });
      this.unlistenSync = await listen("sync-progress", (event) => {
        this.syncProgress = {
          status: event.payload.status,
          progress: event.payload.progress
        };
      });
      this.initialize();
    }
  },
  beforeUnmount() {
    if (this.unlisten) this.unlisten();
    if (this.unlistenSync) this.unlistenSync();
  },
  methods: {
    async triggerSync() {
      this.syncing = true;
      try {
        await invoke("request_start_sync");
      } catch (err) {
        console.error("Failed to start sync:", err);
        this.syncing = false;
      }
    },
    async disconnectSession() {
      this.disconnecting = true;
      try {
        await invoke("stop_webrtc_session");
        this.connectionStatus = this.$t('connect.disconnected');
        this.isConnected = false;
        this.peerJoined = false;
        this.syncProgress = { status: "", progress: 0 };
      } catch (error) {
        console.error("Disconnect Error:", error);
      } finally {
        this.disconnecting = false;
      }
    },
    async confirmLink() {
      this.loading = true;
      const roomId = await invoke("hash_pairing_code", { input: this.uuid });
      // On the host side, start_webrtc_session was already called in listen(roomId)
      // with isInitiator: false.
      // However, we need to ensure the backend actually proceeds with negotiation.
      // Currently, the backend starts as soon as we call start_webrtc_session.
      // To strictly follow "Host must click Link", we would need to delay the backend
      // or send a signaling message. 
      // For now, let's keep it simple: Host clicking "Link" just hides the approval UI.
      this.isConnected = true;
      this.peerJoined = false;
      this.loading = false;
    },
    async initialize() {
        this.connectionStatus = this.$t('connect.generating_key');
        try {
          const codes = await invoke("generate_pairing_codes");
          this.uuid = codes.uuid;
          this.passphrase = codes.passphrase;
          const roomId = await invoke("hash_pairing_code", { input: this.uuid });
          this.listen(roomId);
        } catch (error) {
           console.error("Pairing Error:", error);
           this.connectionStatus = this.$t('connect.pairing_error');
        }
    },
    async listen(roomId) {
      this.connectionStatus = this.$t('connect.waiting_for_partner');
      const signalingUrl = import.meta.env.VITE_SIGNALING_URL || "wss://siegu.io/ws";
      const args = {
          roomId: roomId,
          isInitiator: false,
          signalingUrl: signalingUrl
      };
      try {
          await invoke("start_webrtc_session", args);
          this.connectionStatus = this.$t('connect.awaiting_webrtc');
      } catch (error) {
          this.connectionStatus = this.$t('connect.error_connecting', { error });
      }
    },
    async joinWebRTC() {
        if (!this.joinPassphrase || this.loading) return;
        this.loading = true;
        this.connectionStatus = this.$t('connect.joining_room');
        const signalingUrl = import.meta.env.VITE_SIGNALING_URL || "wss://siegu.io/ws";
        try {
           const roomId = await invoke("hash_pairing_code", { input: this.joinPassphrase });
           const args = {
              roomId: roomId,
              isInitiator: true,
              signalingUrl: signalingUrl
           };
           await invoke("start_webrtc_session", args);
           this.connectionStatus = this.$t('connect.awaiting_webrtc_receiver');
        } catch(error) {
           this.loading = false;
           this.connectionStatus = this.$t('connect.error_joining', { error });
        }
    },
    async startScanner() {
      this.isScanning = true;
      try {
        const videoElement = this.$refs.scannerVideo;
        await this.codeReader.decodeFromVideoDevice(undefined, videoElement, (result, error) => {
          if (result) {
            this.joinPassphrase = result.getText();
            this.stopScanner();
            this.joinWebRTC();
          }
        });
      } catch (err) {
        console.error("Scanner Error:", err);
        this.isScanning = false;
      }
    },
    stopScanner() {
      this.codeReader.reset();
      this.isScanning = false;
    }
  },
};
</script>
