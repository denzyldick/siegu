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
        class="border-subtle pa-6 text-center bg-siegu-white"
        rounded="xl"
        min-width="350"
        max-width="400"
      >
        <div class="text-h5 font-weight-bold text-zinc-primary mb-2">
          {{ $t('connect.link_device_title') }}
        </div>
        <div class="text-body-2 text-zinc-secondary mb-6">
          {{ $t('connect.link_device_desc') }}
        </div>

        <div v-if="!started" class="d-flex flex-column align-center mb-6 ga-4">
          <v-btn
            color="black"
            variant="flat"
            height="56"
            class="siegu-btn px-8 text-none"
            @click="start('host')"
          >
            <div class="d-flex align-center">
              <div class="siegu-icon-circle siegu-icon-circle-md mr-3">
                <v-icon size="16">mdi-laptop</v-icon>
              </div>
              <span class="font-weight-bold">{{ $t('connect.host') }}</span>
            </div>
          </v-btn>
          <div class="text-caption text-zinc-muted text-center px-4">
            {{ $t('connect.host_desc') }}
          </div>
          <v-btn
            color="black"
            variant="flat"
            height="56"
            class="siegu-btn px-8 text-none"
            @click="start('join')"
          >
            <div class="d-flex align-center">
              <div class="siegu-icon-circle siegu-icon-circle-md mr-3">
                <v-icon size="16">mdi-cellphone-link</v-icon>
              </div>
              <span class="font-weight-bold">{{ $t('connect.join') }}</span>
            </div>
          </v-btn>
          <div class="text-caption text-zinc-muted text-center px-4">
            {{ $t('connect.join_desc') }}
          </div>
        </div>

        <template v-if="started">
          <ConnectModeToggle v-if="!hideModeToggle" v-model="mode" />

          <ConnectHostView
            v-if="mode === 'host'"
            :uuid="uuid"
            :passphrase="passphrase"
            :peer-joined="peerJoined"
            :is-connected="isConnected"
            :host-ip="hostIp"
            :host-port="hostPort"
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
            @disconnect="disconnectSession"
          />
        </template>

        <v-divider class="opacity-10 my-4"></v-divider>

        <v-card-actions class="justify-center">
          <v-btn variant="flat" color="#18181b" class="siegu-btn px-6" @click="dialog = false">
            <div class="d-flex align-center">
              <div class="siegu-icon-circle siegu-icon-circle-sm mr-2">
                <v-icon size="12">mdi-close</v-icon>
              </div>
              <span class="font-weight-bold">{{ $t('common.close') }}</span>
            </div>
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <div v-if="embedded" class="w-100">
      <div v-if="!started && !hideModeToggle" class="d-flex flex-column align-center mb-6 ga-4">
        <v-btn
          color="black"
          variant="flat"
          height="56"
          class="siegu-btn px-8 text-none"
          @click="start('host')"
        >
          <div class="d-flex align-center">
            <div class="siegu-icon-circle siegu-icon-circle-md mr-3">
              <v-icon size="16">mdi-laptop</v-icon>
            </div>
            <span class="font-weight-bold">{{ $t('connect.host') }}</span>
          </div>
        </v-btn>
        <div class="text-caption text-zinc-muted text-center px-4">
          {{ $t('connect.host_desc') }}
        </div>
        <v-btn
          color="black"
          variant="flat"
          height="56"
          class="siegu-btn px-8 text-none"
          @click="start('join')"
        >
          <div class="d-flex align-center">
            <div class="siegu-icon-circle siegu-icon-circle-md mr-3">
              <v-icon size="16">mdi-cellphone-link</v-icon>
            </div>
            <span class="font-weight-bold">{{ $t('connect.join') }}</span>
          </div>
        </v-btn>
        <div class="text-caption text-zinc-muted text-center px-4">
          {{ $t('connect.join_desc') }}
        </div>
      </div>

      <template v-if="started">
        <ConnectModeToggle v-if="!hideModeToggle" v-model="mode" />

        <ConnectHostView
          v-if="mode === 'host'"
          :uuid="uuid"
          :passphrase="passphrase"
          :peer-joined="peerJoined"
          :is-connected="isConnected"
          :host-ip="hostIp"
          :host-port="hostPort"
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
import ConnectModeToggle from '@/components/connect/ConnectModeToggle.vue'
import ConnectHostView from '@/components/connect/ConnectHostView.vue'
import ConnectJoinView from '@/components/connect/ConnectJoinView.vue'
import ConnectLanDiscovery from '@/components/connect/ConnectLanDiscovery.vue'
import ConnectStatusBar from '@/components/connect/ConnectStatusBar.vue'

const props = withDefaults(
  defineProps<{
    embedded?: boolean
    initialMode?: 'host' | 'join'
    hideModeToggle?: boolean
  }>(),
  {
    embedded: false,
    initialMode: 'host',
    hideModeToggle: false,
  },
)

const emit = defineEmits<{
  connected: []
  modeChange: [mode: 'host' | 'join']
  done: []
}>()

const dialog = ref(false)
const started = ref(false)
const {
  mode,
  uuid,
  passphrase,
  joinPassphrase,
  connectionStatus,
  isConnected,
  peerJoined,
  loading,
  syncing,
  disconnecting,
  syncProgress,
  selectedLanHost,
  peerList,
  hostIp,
  hostPort,
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
  disconnectSession()
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
