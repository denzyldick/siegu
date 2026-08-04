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
        <div
          v-if="!confirmDialog && !(started && mode === 'host' && !isConnected)"
          class="d-flex justify-center mb-3"
        >
          <v-icon
            size="72"
            class="connect-illustration"
            color="#22c55e"
          >mdi-lan-connect</v-icon>
        </div>

        <div
          v-if="!confirmDialog && !(started && mode === 'host' && !isConnected)"
          class="text-h5 font-weight-bold text-zinc-primary mb-2"
        >
          {{ $t('connect.link_device_title') }}
        </div>
        <div
          v-if="
            !confirmDialog && !(started && (mode === 'host' || mode === 'join') && !isConnected)
          "
          class="text-body-2 text-zinc-secondary mb-6"
        >
          {{ $t('connect.link_device_desc') }}
        </div>
        <div
          v-if="
            !confirmDialog && !(started && (mode === 'host' || mode === 'join') && !isConnected)
          "
          class="text-caption text-zinc-muted mb-6 px-2"
          style="line-height: 1.5"
        >
          {{ $t('connect.privacy_note') }}
        </div>

        <div
          v-if="!started && !confirmDialog"
          class="d-flex flex-row justify-center mb-2 ga-3 w-100"
        >
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
            style="color: #22c55e; text-decoration: none"
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
              color="primary"
              class="text-none flex-1"
              @click="confirmDialog = false"
            >
              Back
            </v-btn>
            <v-btn
              color="primary"
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

          <div v-if="peerList.length > 0" class="text-left px-2 mb-2">
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

        <div v-if="peerList.length > 0" class="text-left px-2 mb-2">
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
import { ref, watch, computed, onMounted, onBeforeUnmount } from 'vue';
import { useConnect } from '@/composables/useConnect';
import ConnectHostView from '@/components/connect/ConnectHostView.vue';
import ConnectJoinView from '@/components/connect/ConnectJoinView.vue';
import ConnectLanDiscovery from '@/components/connect/ConnectLanDiscovery.vue';
import ConnectStatusBar from '@/components/connect/ConnectStatusBar.vue';

const props = withDefaults(
  defineProps<{
    embedded?: boolean;
    initialMode?: 'host' | 'join';
    hideModeToggle?: boolean;
    keepSessionOnUnmount?: boolean;
  }>(),
  {
    embedded: false,
    initialMode: 'host',
    hideModeToggle: false,
    keepSessionOnUnmount: false,
  },
);

const emit = defineEmits<{
  connected: [];
  modeChange: [mode: 'host' | 'join'];
  done: [];
}>();

const dialog = ref(false);
const started = ref(false);
const confirmDialog = ref(false);
const pendingMode = ref<'host' | 'join' | null>(null);
const dontShowAgain = ref(localStorage.getItem('siegu_skip_network_confirm') === 'true');

function confirmStart(selectedMode: 'host' | 'join') {
  if (dontShowAgain.value) {
    start(selectedMode);
  } else {
    pendingMode.value = selectedMode;
    confirmDialog.value = true;
  }
}

function proceedFromConfirm() {
  if (dontShowAgain.value) {
    localStorage.setItem('siegu_skip_network_confirm', 'true');
  }
  confirmDialog.value = false;
  if (pendingMode.value) {
    start(pendingMode.value);
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
} = useConnect();

const showDisconnect = computed(() => {
  return (
    !props.embedded &&
    (isConnected.value ||
      (connectionStatus.value.length > 0 && connectionStatus.value !== 'Disconnected'))
  );
});

async function handleDisconnect() {
  await disconnectSession();
  dialog.value = false;
}

function start(selectedMode: 'host' | 'join') {
  started.value = true;
  mode.value = selectedMode;
  if (selectedMode === 'host') {
    initialize();
  }
}

watch(mode, async (newMode, oldMode) => {
  emit('modeChange', newMode);
  if (started.value && oldMode && oldMode !== newMode) {
    await disconnectSession();
    resetJoinState();
    if (newMode === 'host') {
      initialize();
    }
  }
});

watch(isConnected, (connected) => {
  if (connected) emit('connected');
});

watch(dialog, async (open) => {
  if (open) {
    await startEventListeners();
  } else {
    started.value = false;
    confirmDialog.value = false;
    pendingMode.value = null;
    await disconnectSession();
    stopEventListeners();
    loading.value = false;
  }
});

onMounted(() => {
  mode.value = props.initialMode;
  if (props.embedded) {
    startEventListeners();
    if (props.hideModeToggle) {
      start(props.initialMode);
    }
  }
});

onBeforeUnmount(() => {
  if (!props.keepSessionOnUnmount) {
    disconnectSession();
  }
  stopEventListeners();
});
</script>

<style scoped>
.connect-illustration {
  opacity: 0.9;
}
</style>
