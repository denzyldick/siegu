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

        <ConnectModeToggle v-if="!hideModeToggle" v-model="mode" />

        <ConnectHostView
          v-if="mode === 'host'"
          :uuid="uuid"
          :passphrase="passphrase"
          :peer-joined="peerJoined"
          :is-connected="isConnected"
        />

        <ConnectJoinView
          v-if="mode === 'join' && selectedLanHost"
          v-model="joinPassphrase"
          :loading="loading"
          :is-connected="isConnected"
          :show-sync-button="isConnected"
          :syncing="syncing"
          @join="joinWebRTC"
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

        <ConnectStatusBar
          :status="connectionStatus"
          :is-connected="isConnected"
          :sync-progress="syncProgress"
          :show-disconnect="showDisconnect"
          :disconnecting="disconnecting"
          @disconnect="disconnectSession"
        />

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
      <ConnectModeToggle v-if="!hideModeToggle" v-model="mode" />

      <ConnectHostView
        v-if="mode === 'host'"
        :uuid="uuid"
        :passphrase="passphrase"
        :peer-joined="peerJoined"
        :is-connected="isConnected"
      />

      <ConnectJoinView
        v-if="mode === 'join' && selectedLanHost"
        v-model="joinPassphrase"
        :loading="loading"
        :is-connected="isConnected"
        :show-sync-button="false"
        :syncing="syncing"
        @join="joinWebRTC"
        @sync="triggerSync"
      />

      <ConnectLanDiscovery
        v-if="mode === 'join' && !selectedLanHost && !isConnected"
        @select="selectLanHost"
      />

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
import ConnectSyncButton from '@/components/connect/ConnectSyncButton.vue'
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
  initialize,
  selectLanHost,
  joinWebRTC,
  triggerSync,
  disconnectSession,
  startEventListeners,
  stopEventListeners,
  resetJoinState,
} = useConnect()

let syncCompleteTimer: ReturnType<typeof setTimeout> | null = null

const showDisconnect = computed(() => {
  return (
    !props.embedded &&
    (isConnected.value ||
      (connectionStatus.value.length > 0 && connectionStatus.value !== 'Disconnected'))
  )
})

watch(mode, (newMode) => {
  emit('modeChange', newMode)
  if (newMode === 'host' && !uuid.value) {
    initialize()
  } else if (newMode === 'join') {
    resetJoinState()
  }
})

watch(isConnected, (connected) => {
  if (connected) emit('connected')
})

watch(syncProgress, (progress) => {
  if (syncCompleteTimer) {
    clearTimeout(syncCompleteTimer)
    syncCompleteTimer = null
  }
  if (progress.status === 'Up to date' || progress.status.startsWith('Finished')) {
    const currentStatus = progress.status
    syncCompleteTimer = setTimeout(() => {
      if (syncProgress.value.status === currentStatus) {
        syncProgress.value = { status: '', progress: 0 }
        if (dialog.value) {
          dialog.value = false
          emit('done')
        }
      }
    }, 2000)
  }
})

watch(dialog, async (open) => {
  if (open) {
    await startEventListeners()
    initialize()
  } else {
    stopEventListeners()
    loading.value = false
    if (syncCompleteTimer) {
      clearTimeout(syncCompleteTimer)
      syncCompleteTimer = null
    }
  }
})

onMounted(() => {
  mode.value = props.initialMode
  if (props.embedded) {
    startEventListeners()
    initialize()
  }
})

onBeforeUnmount(() => {
  if (syncCompleteTimer) {
    clearTimeout(syncCompleteTimer)
  }
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
