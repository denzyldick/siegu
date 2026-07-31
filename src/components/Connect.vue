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
              <v-icon size="14">mdi-plus</v-icon>
            </div>
            <span class="font-weight-bold">{{ $t('devices.add_device') }}</span>
          </div>
        </v-btn>
      </template>

      <v-card
        v-if="!embedded"
        class="border-subtle pa-5 text-center bg-siegu-white"
        rounded="xl"
        min-width="350"
        max-width="440"
      >
        <div v-if="!confirmDialog && !(started && mode === 'host' && !isConnected)" class="d-flex justify-center mb-3">
          <svg viewBox="0 0 240 90" width="180" height="68" class="connect-illustration">
            <!-- Desktop -->
            <rect x="8" y="12" width="52" height="38" rx="3" fill="none" stroke="#18181b" stroke-width="2"/>
            <rect x="13" y="17" width="42" height="28" rx="1" fill="#18181b" opacity="0.06"/>
            <rect x="8" y="50" width="52" height="5" rx="1" fill="none" stroke="#18181b" stroke-width="2"/>
            <rect x="13" y="55" width="42" height="3" rx="1" fill="none" stroke="#18181b" stroke-width="1.5"/>
            <!-- Lock on desktop -->
            <rect x="30" y="24" width="8" height="6" rx="1.5" fill="none" stroke="#22c55e" stroke-width="1.5"/>
            <path d="M30,27 v-3 a4,4 0 0,1 8,0 v3" fill="none" stroke="#22c55e" stroke-width="1.5"/>
            <circle cx="34" cy="29" r="1.2" fill="#22c55e"/>

            <!-- Direct connection line -->
            <line x1="64" y1="35" x2="166" y2="35" stroke="#18181b" stroke-width="1.5" stroke-dasharray="4,3" opacity="0.3"/>
            <line x1="64" y1="45" x2="166" y2="45" stroke="#18181b" stroke-width="1" stroke-dasharray="3,4" opacity="0.15"/>

            <!-- Animated data dots -->
            <circle r="4" fill="#22c55e" opacity="0.8">
              <animateMotion dur="2.5s" repeatCount="indefinite" path="M64,35 L166,35"/>
            </circle>
            <circle r="3" fill="#22c55e" opacity="0.5">
              <animateMotion dur="2.5s" repeatCount="indefinite" begin="0.8s" path="M64,35 L166,35"/>
            </circle>
            <circle r="2" fill="#22c55e" opacity="0.3">
              <animateMotion dur="2.5s" repeatCount="indefinite" begin="1.6s" path="M64,35 L166,35"/>
            </circle>

            <!-- Reverse data dot -->
            <circle r="3" fill="#a855f7" opacity="0.5">
              <animateMotion dur="3s" repeatCount="indefinite" path="M166,45 L64,45"/>
            </circle>

            <!-- Phone -->
            <rect x="175" y="10" width="24" height="44" rx="4" fill="none" stroke="#18181b" stroke-width="2"/>
            <rect x="179" y="15" width="16" height="24" rx="1" fill="#18181b" opacity="0.06"/>
            <circle cx="187" cy="48" r="3" fill="none" stroke="#18181b" stroke-width="1.5"/>
            <rect x="183" y="11.5" width="8" height="1.5" rx="0.75" fill="#18181b" opacity="0.3"/>
            <!-- Check on phone -->
            <path d="M183,26 l3,3 l6,-6" fill="none" stroke="#22c55e" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>

            <!-- Labels -->
            <text x="34" y="72" text-anchor="middle" fill="#71717a" font-size="8" font-family="inherit">This device</text>
            <text x="187" y="72" text-anchor="middle" fill="#71717a" font-size="8" font-family="inherit">Other device</text>
          </svg>
        </div>

        <div v-if="!confirmDialog && !(started && mode === 'host' && !isConnected)" class="text-h5 font-weight-bold text-zinc-primary mb-2">
          {{ $t('connect.link_device_title') }}
        </div>
        <div v-if="!confirmDialog && !(started && (mode === 'host' || mode === 'join') && !isConnected)" class="text-body-2 text-zinc-secondary mb-6">
          {{ $t('connect.link_device_desc') }}
        </div>
        <div v-if="!confirmDialog && !(started && (mode === 'host' || mode === 'join') && !isConnected)" class="text-caption text-zinc-muted mb-6 px-2" style="line-height: 1.5">
          {{ $t('connect.privacy_note') }}
        </div>

        <div v-if="!started && !confirmDialog" class="d-flex flex-row justify-center mb-2 ga-3 w-100">
          <v-tooltip :text="$t('connect.host_desc')" location="bottom">
            <template v-slot:activator="{ props }">
              <v-btn
                v-bind="props"
                color="black"
                variant="flat"
                height="44"
                class="siegu-btn px-5 text-none"
                @click="confirmStart('host')"
              >
                <div class="d-flex align-center">
                  <div class="siegu-icon-circle siegu-icon-circle-sm mr-2">
                    <v-icon size="14">mdi-laptop</v-icon>
                  </div>
                  <span class="font-weight-bold">{{ $t('connect.host') }}</span>
                </div>
              </v-btn>
            </template>
          </v-tooltip>
          <v-tooltip :text="$t('connect.join_desc')" location="bottom">
            <template v-slot:activator="{ props }">
              <v-btn
                v-bind="props"
                color="black"
                variant="flat"
                height="44"
                class="siegu-btn px-5 text-none"
                @click="confirmStart('join')"
              >
                <div class="d-flex align-center">
                  <div class="siegu-icon-circle siegu-icon-circle-sm mr-2">
                    <v-icon size="14">mdi-cellphone-link</v-icon>
                  </div>
                  <span class="font-weight-bold">{{ $t('connect.join') }}</span>
                </div>
              </v-btn>
            </template>
          </v-tooltip>
        </div>

        <div v-if="!started && confirmDialog" class="text-left px-2">
          <div class="text-body-2 font-weight-bold text-zinc-primary mb-3">
            {{ $t('connect.network_confirm_title') }}
          </div>
          <div class="text-caption text-zinc-secondary mb-3" style="line-height: 1.5">
            {{ $t('connect.same_network_note') }}
          </div>
          <a
            href="https://siegu.app/waitlist"
            target="_blank"
            class="text-caption font-weight-medium mb-4 d-inline-block"
            style="color: #22c55e; text-decoration: none;"
          >
            {{ $t('connect.join_waitlist') }} →
          </a>
          <v-checkbox
            v-model="dontShowAgain"
            :label="$t('connect.network_confirm_dont_show')"
            hide-details
            density="compact"
            class="text-caption mb-4"
            color="black"
          />
          <div class="d-flex ga-3">
            <v-btn
              variant="outlined"
              color="black"
              class="text-none flex-1"
              @click="confirmDialog = false"
            >
              Back
            </v-btn>
            <v-btn
              color="black"
              variant="flat"
              class="text-none flex-1"
              @click="proceedFromConfirm"
            >
              Continue
            </v-btn>
          </div>
        </div>

        <template v-if="started">
          <ConnectHostView
            v-if="mode === 'host'"
            :passphrase="passphrase"
            :is-connected="isConnected"
            :syncing="syncing"
            :items-completed="syncProgress.items_completed"
            :items-total="syncProgress.items_total"
            :peer-name="peerList[0]?.name ?? ''"
          />

          <ConnectJoinView
            v-if="mode === 'join'"
            v-model="joinPassphrase"
            :loading="loading"
            :is-connected="isConnected"
            :show-sync-button="isConnected"
            :syncing="syncing"
            :host-ip="selectedLanHost?.ip ?? ''"
            :host-port="selectedLanHost?.port ?? 0"
            :device-name="selectedLanHost?.name ?? ''"
            :items-completed="syncProgress.items_completed"
            :items-total="syncProgress.items_total"
            :connection-status="connectionStatus"
            @join="(ip: string, port: string) => joinWebRTC(ip, port)"
            @sync="triggerSync"
          />

          <ConnectLanDiscovery
            v-if="mode === 'join' && !selectedLanHost && !isConnected"
            @select="selectLanHost"
          />

          <div
            v-if="mode === 'host' && isConnected && syncing"
            class="d-flex justify-center mb-6"
          >
            <v-progress-linear
              indeterminate
              color="success"
              height="4"
            />
          </div>

          <div
            v-if="peerList.length > 0"
            class="text-left px-2 mb-2"
          >
            <div class="text-caption font-weight-bold text-zinc-secondary mb-1">
              {{ $t('devices.connected') }} ({{ peerList.length }})
            </div>
            <div
              v-for="peer in peerList"
              :key="peer.device_id"
              class="d-flex align-center pa-2 mb-1 rounded bg-zinc-50"
            >
              <v-icon size="14" class="mr-2 text-zinc-secondary">mdi-laptop</v-icon>
              <span class="text-body-2 text-zinc-primary font-weight-medium">{{ peer.name }}</span>
              <span class="text-caption text-zinc-secondary ml-2">{{ peer.os }}</span>
            </div>
          </div>

          <ConnectStatusBar
            :status="connectionStatus"
            :is-connected="isConnected"
            :sync-progress="syncProgress"
            :show-disconnect="showDisconnect"
            :disconnecting="disconnecting"
            @disconnect="handleDisconnect"
          />
        </template>

      </v-card>
    </v-dialog>

    <div v-if="embedded" class="w-100">
      <div v-if="!started && !hideModeToggle" class="d-flex flex-row align-center mb-6 ga-3 w-100">
        <v-tooltip :text="$t('connect.host_desc')" location="bottom">
          <template v-slot:activator="{ props }">
            <v-btn
              v-bind="props"
              color="black"
              variant="flat"
              height="56"
              class="siegu-btn px-6 text-none flex-1"
              @click="start('host')"
            >
              <div class="d-flex align-center">
                <div class="siegu-icon-circle siegu-icon-circle-md mr-2">
                  <v-icon size="16">mdi-laptop</v-icon>
                </div>
                <span class="font-weight-bold">{{ $t('connect.host') }}</span>
              </div>
            </v-btn>
          </template>
        </v-tooltip>
        <v-tooltip :text="$t('connect.join_desc')" location="bottom">
          <template v-slot:activator="{ props }">
            <v-btn
              v-bind="props"
              color="black"
              variant="flat"
              height="56"
              class="siegu-btn px-6 text-none flex-1"
              @click="start('join')"
            >
              <div class="d-flex align-center">
                <div class="siegu-icon-circle siegu-icon-circle-md mr-2">
                  <v-icon size="16">mdi-cellphone-link</v-icon>
                </div>
                <span class="font-weight-bold">{{ $t('connect.join') }}</span>
              </div>
            </v-btn>
          </template>
        </v-tooltip>
      </div>

      <template v-if="started">
        <ConnectHostView
          v-if="mode === 'host'"
          :passphrase="passphrase"
          :is-connected="isConnected"
          :syncing="syncing"
          :items-completed="syncProgress.items_completed"
          :items-total="syncProgress.items_total"
          :peer-name="peerList[0]?.name ?? ''"
        />

        <ConnectJoinView
          v-if="mode === 'join'"
          v-model="joinPassphrase"
          :loading="loading"
          :is-connected="isConnected"
          :show-sync-button="false"
          :syncing="syncing"
          :host-ip="selectedLanHost?.ip ?? ''"
          :host-port="selectedLanHost?.port ?? 0"
          :device-name="selectedLanHost?.name ?? ''"
          :items-completed="syncProgress.items_completed"
          :items-total="syncProgress.items_total"
          :connection-status="connectionStatus"
          @join="(ip: string, port: string) => joinWebRTC(ip, port)"
          @sync="triggerSync"
        />

        <ConnectLanDiscovery
          v-if="mode === 'join' && !selectedLanHost && !isConnected"
          @select="selectLanHost"
        />

        <div
          v-if="peerList.length > 0"
          class="text-left px-2 mb-2"
        >
          <div class="text-caption font-weight-bold text-zinc-secondary mb-1">
            {{ $t('devices.connected') }} ({{ peerList.length }})
          </div>
          <div
            v-for="peer in peerList"
            :key="peer.device_id"
            class="d-flex align-center pa-2 mb-1 rounded bg-zinc-50"
          >
            <v-icon size="14" class="mr-2 text-zinc-secondary">mdi-laptop</v-icon>
          <span class="text-body-2 text-zinc-primary font-weight-medium">{{ peer.name }}</span>
          <span class="text-caption text-zinc-secondary ml-2">{{ peer.os }}</span>
        </div>
      </div>

      <div class="text-caption text-zinc-muted mb-1 text-center py-2" v-if="connectionStatus">
        <v-progress-circular
          v-if="!isConnected"
          indeterminate
          color="black"
          size="16"
          width="2"
          class="mr-2 opacity-50"
        ></v-progress-circular>
        <v-icon v-else color="success" size="16" class="mr-2">mdi-check-circle-outline</v-icon>
        {{ connectionStatus }}
      </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onBeforeUnmount } from 'vue'
import { useConnect } from '@/composables/useConnect'
import ConnectHostView from '@/components/connect/ConnectHostView.vue'
import ConnectJoinView from '@/components/connect/ConnectJoinView.vue'
import ConnectLanDiscovery from '@/components/connect/ConnectLanDiscovery.vue'
import ConnectStatusBar from '@/components/connect/ConnectStatusBar.vue'

const props = withDefaults(
  defineProps<{
    embedded?: boolean
    initialMode?: 'host' | 'join'
    hideModeToggle?: boolean
    keepSessionOnUnmount?: boolean
  }>(),
  {
    embedded: false,
    initialMode: 'host',
    hideModeToggle: false,
    keepSessionOnUnmount: false,
  },
)

const emit = defineEmits<{
  connected: []
  modeChange: [mode: 'host' | 'join']
  done: []
}>()

const dialog = ref(false)
const started = ref(false)
const confirmDialog = ref(false)
const pendingMode = ref<'host' | 'join' | null>(null)
const dontShowAgain = ref(localStorage.getItem('siegu_skip_network_confirm') === 'true')

function confirmStart(selectedMode: 'host' | 'join') {
  if (dontShowAgain.value) {
    start(selectedMode)
  } else {
    pendingMode.value = selectedMode
    confirmDialog.value = true
  }
}

function proceedFromConfirm() {
  if (dontShowAgain.value) {
    localStorage.setItem('siegu_skip_network_confirm', 'true')
  }
  confirmDialog.value = false
  if (pendingMode.value) {
    start(pendingMode.value)
  }
}
const {
  mode,
  passphrase,
  joinPassphrase,
  connectionStatus,
  isConnected,
  loading,
  syncing,
  disconnecting,
  syncProgress,
  selectedLanHost,
  peerList,
  initialize,
  selectLanHost,
  joinWebRTC,
  triggerSync,
  disconnectSession,
  startEventListeners,
  stopEventListeners,
  resetJoinState,
} = useConnect()

const showDisconnect = computed(() => {
  return (
    !props.embedded &&
    (isConnected.value ||
      (connectionStatus.value.length > 0 && connectionStatus.value !== 'Disconnected'))
  )
})

async function handleDisconnect() {
  await disconnectSession()
  dialog.value = false
}

function start(selectedMode: 'host' | 'join') {
  started.value = true
  mode.value = selectedMode
  if (selectedMode === 'host') {
    initialize()
  }
}

watch(mode, async (newMode, oldMode) => {
  emit('modeChange', newMode)
  if (started.value && oldMode && oldMode !== newMode) {
    await disconnectSession()
    resetJoinState()
    if (newMode === 'host') {
      initialize()
    }
  }
})

watch(isConnected, (connected) => {
  if (connected) emit('connected')
})

watch(dialog, async (open) => {
  if (open) {
    await startEventListeners()
  } else {
    started.value = false
    confirmDialog.value = false
    pendingMode.value = null
    await disconnectSession()
    stopEventListeners()
    loading.value = false
  }
})

onMounted(() => {
  mode.value = props.initialMode
  if (props.embedded) {
    startEventListeners()
    if (props.hideModeToggle) {
      start(props.initialMode)
    }
  }
})

onBeforeUnmount(() => {
  if (!props.keepSessionOnUnmount) {
    disconnectSession()
  }
  stopEventListeners()
})
</script>

<style scoped>
.siegu-icon-circle {
  width: 28px;
  height: 28px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.siegu-icon-circle-md {
  width: 32px;
  height: 32px;
}

.siegu-icon-circle-sm {
  width: 22px;
  height: 22px;
}
</style>
