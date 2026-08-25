<template>
  <div class="connect-wrapper">
    <v-dialog v-model="dialog" width="auto" scrim="black" transition="dialog-bottom-transition">
      <template v-slot:activator="{ props }">
        <v-tooltip :text="$t('devices.add_device')" location="top">
          <template v-slot:activator="{ props: tipProps }">
            <v-btn
              v-if="!embedded"
              v-bind="{ ...props, ...tipProps }"
              icon
              color="primary"
              variant="flat"
              size="40"
            >
              <v-icon size="20">mdi-link-plus</v-icon>
            </v-btn>
          </template>
        </v-tooltip>
      </template>

      <v-card
        v-if="!embedded"
        class="border pa-5 text-center"
        rounded="xl"
        min-width="350"
        max-width="440"
      >
        <div
          v-if="!confirmDialog && !(started && mode === 'host' && !isConnected)"
          class="d-flex justify-center mb-3"
        >
          <v-icon size="72" class="connect-illustration" color="rgb(var(--v-theme-success))"
            >mdi-lan-connect</v-icon
          >
        </div>

        <div
          v-if="!confirmDialog && !(started && mode === 'host' && !isConnected)"
          class="text-h5 font-weight-bold text-high-emphasis mb-2"
        >
          {{ $t('connect.link_device_title') }}
        </div>
        <div
          v-if="
            !confirmDialog && !(started && (mode === 'host' || mode === 'join') && !isConnected)
          "
          class="text-body-2 text-medium-emphasis mb-6"
        >
          {{ $t('connect.link_device_desc') }}
        </div>
        <div
          v-if="
            !confirmDialog && !(started && (mode === 'host' || mode === 'join') && !isConnected)
          "
          class="text-caption text-disabled mb-6 px-2"
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
                color="primary"
                variant="flat"
                height="44"
                class="px-5 text-none"
                @click="confirmStart('host')"
              >
                <div class="d-flex align-center">
                  <v-avatar color="rgba(var(--v-theme-on-primary), 0.2)" size="28" class="mr-2">
                    <v-icon size="14">mdi-laptop</v-icon>
                  </v-avatar>
                  <span class="font-weight-bold">{{ $t('connect.host') }}</span>
                </div>
              </v-btn>
            </template>
          </v-tooltip>
          <v-tooltip :text="$t('connect.join_desc')" location="bottom">
            <template v-slot:activator="{ props }">
              <v-btn
                v-bind="props"
                color="primary"
                variant="flat"
                height="44"
                class="px-5 text-none"
                @click="confirmStart('join')"
              >
                <div class="d-flex align-center">
                  <v-avatar color="rgba(var(--v-theme-on-primary), 0.2)" size="28" class="mr-2">
                    <v-icon size="14">mdi-cellphone-link</v-icon>
                  </v-avatar>
                  <span class="font-weight-bold">{{ $t('connect.join') }}</span>
                </div>
              </v-btn>
            </template>
          </v-tooltip>
        </div>

        <div v-if="!started && confirmDialog" class="text-left px-2">
          <div class="text-body-2 font-weight-bold text-high-emphasis mb-3">
            {{ $t('connect.network_confirm_title') }}
          </div>
          <div class="text-caption text-medium-emphasis mb-3" style="line-height: 1.5">
            {{ $t('connect.same_network_note') }}
          </div>
          <a
            href="https://siegu.app/waitlist"
            target="_blank"
            class="text-caption font-weight-medium mb-4 d-inline-block"
            style="color: rgb(var(--v-theme-success)); text-decoration: none"
          >
            {{ $t('connect.join_waitlist') }} →
          </a>
          <v-checkbox
            v-model="dontShowAgain"
            :label="$t('connect.network_confirm_dont_show')"
            hide-details
            density="compact"
            class="text-caption mb-4"
            color="rgb(var(--v-theme-on-surface))"
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
          <GuestGallery v-if="syncStore.viewOnlyActive" />

          <template v-else>
            <ConnectHostView
              v-if="mode === 'host'"
              :passphrase="passphrase"
              :is-connected="isConnected"
              :syncing="syncing"
              :progress="syncProgress.progress"
              :items-completed="syncProgress.items_completed"
              :items-total="syncProgress.items_total"
              :peer-name="peerList[0]?.name ?? ''"
              :peer-os="peerList[0]?.os ?? ''"
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
              :device-os="peerList[0]?.os ?? ''"
              :items-completed="syncProgress.items_completed"
              :items-total="syncProgress.items_total"
              :progress="syncProgress.progress"
              :connection-status="connectionStatus"
              @join="(ip: string, port: string) => joinWebRTC(ip, port)"
              @sync="triggerSync"
            />

            <ConnectLanDiscovery
              v-if="mode === 'join' && !selectedLanHost && !isConnected"
              @select="selectLanHost"
            />
          </template>

          <div v-if="peerList.length > 1" class="text-left px-2 mb-2">
            <div class="text-caption font-weight-bold text-medium-emphasis mb-1">
              {{ $t('devices.connected') }} ({{ peerList.length }})
            </div>
            <div
              v-for="peer in peerList"
              :key="peer.device_id"
              style="background: rgb(var(--v-theme-surface))"
              class="d-flex align-center pa-2 mb-1 rounded"
            >
              <v-icon size="14" class="mr-2 text-medium-emphasis">{{
                deviceOsIcon(peer.os)
              }}</v-icon>
              <span class="text-body-2 text-high-emphasis font-weight-medium">{{ peer.name }}</span>
              <span class="text-caption text-medium-emphasis ml-2">{{ peer.os }}</span>
            </div>
          </div>

          <v-btn
            v-if="isConnected && !syncStore.viewOnlyActive"
            variant="tonal"
            color="secondary"
            size="small"
            class="text-none mb-2 mx-2 align-self-center"
            prepend-icon="mdi-eye-outline"
            @click="syncStore.browseOnly()"
          >
            {{ $t('connect.view_only_browse') }}
          </v-btn>

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
              color="primary"
              variant="flat"
              height="56"
              class="px-6 text-none flex-1"
              @click="start('host')"
            >
              <div class="d-flex align-center">
                <v-avatar color="rgba(var(--v-theme-on-primary), 0.2)" size="32" class="mr-2">
                  <v-icon size="16">mdi-laptop</v-icon>
                </v-avatar>
                <span class="font-weight-bold">{{ $t('connect.host') }}</span>
              </div>
            </v-btn>
          </template>
        </v-tooltip>
        <v-tooltip :text="$t('connect.join_desc')" location="bottom">
          <template v-slot:activator="{ props }">
            <v-btn
              v-bind="props"
              color="primary"
              variant="flat"
              height="56"
              class="px-6 text-none flex-1"
              @click="start('join')"
            >
              <div class="d-flex align-center">
                <v-avatar color="rgba(var(--v-theme-on-primary), 0.2)" size="32" class="mr-2">
                  <v-icon size="16">mdi-cellphone-link</v-icon>
                </v-avatar>
                <span class="font-weight-bold">{{ $t('connect.join') }}</span>
              </div>
            </v-btn>
          </template>
        </v-tooltip>
      </div>

      <template v-if="started">
        <GuestGallery v-if="syncStore.viewOnlyActive" />

        <template v-else>
          <ConnectHostView
            v-if="mode === 'host'"
            :passphrase="passphrase"
            :is-connected="isConnected"
            :syncing="syncing"
            :progress="syncProgress.progress"
            :items-completed="syncProgress.items_completed"
            :items-total="syncProgress.items_total"
            :peer-name="peerList[0]?.name ?? ''"
            :peer-os="peerList[0]?.os ?? ''"
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
            :device-os="peerList[0]?.os ?? ''"
            :items-completed="syncProgress.items_completed"
            :items-total="syncProgress.items_total"
            :progress="syncProgress.progress"
            :connection-status="connectionStatus"
            @join="(ip: string, port: string) => joinWebRTC(ip, port)"
            @sync="triggerSync"
          />

          <ConnectLanDiscovery
            v-if="mode === 'join' && !selectedLanHost && !isConnected"
            @select="selectLanHost"
          />
        </template>

        <div v-if="peerList.length > 1" class="text-left px-2 mb-2">
          <div class="text-caption font-weight-bold text-medium-emphasis mb-1">
            {{ $t('devices.connected') }} ({{ peerList.length }})
          </div>
          <div
            v-for="peer in peerList"
            :key="peer.device_id"
            style="background: rgb(var(--v-theme-surface))"
            class="d-flex align-center pa-2 mb-1 rounded"
          >
            <v-icon size="14" class="mr-2 text-medium-emphasis">{{ deviceOsIcon(peer.os) }}</v-icon>
            <span class="text-body-2 text-high-emphasis font-weight-medium">{{ peer.name }}</span>
            <span class="text-caption text-medium-emphasis ml-2">{{ peer.os }}</span>
          </div>
        </div>

        <v-btn
          v-if="isConnected && !syncStore.viewOnlyActive"
          variant="tonal"
          color="secondary"
          size="small"
          class="text-none mb-2 align-self-center"
          prepend-icon="mdi-eye-outline"
          @click="syncStore.browseOnly()"
        >
          {{ $t('connect.view_only_browse') }}
        </v-btn>

        <div
          class="text-caption text-disabled mb-1 text-center py-2"
          v-if="connectionStatus && !isConnected"
        >
          <v-progress-circular
            v-if="!isConnected"
            indeterminate
            color="rgba(var(--v-theme-on-surface), 0.7)"
            size="16"
            width="2"
            class="mr-2 opacity-50"
          ></v-progress-circular>
          <v-icon v-else color="success" size="16" class="mr-2">mdi-check-circle-outline</v-icon>
          {{ displayStatus }}
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onBeforeUnmount } from 'vue';
import { useI18n } from 'vue-i18n';
import { useConnect } from '@/composables/useConnect';
import { useSyncStore } from '@/stores/sync';
import { connectionStatusKey } from '@/utils/connectStatus';
import { deviceOsIcon } from '@/utils/format';
import ConnectHostView from '@/components/connect/ConnectHostView.vue';
import ConnectJoinView from '@/components/connect/ConnectJoinView.vue';
import ConnectLanDiscovery from '@/components/connect/ConnectLanDiscovery.vue';
import ConnectStatusBar from '@/components/connect/ConnectStatusBar.vue';
import GuestGallery from '@/components/connect/GuestGallery.vue';

const { t } = useI18n();
const syncStore = useSyncStore();

const props = withDefaults(
  defineProps<{
    embedded?: boolean;
    initialMode?: 'host' | 'join';
    hideModeToggle?: boolean;
  }>(),
  {
    embedded: false,
    initialMode: 'host',
    hideModeToggle: false,
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

const displayStatus = computed(() => {
  const key = connectionStatusKey(connectionStatus.value);
  return key ? t(key) : connectionStatus.value;
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
    stopEventListeners();
    loading.value = false;
    // NOTE: closing the dialog must NOT tear down the backend session.
    // Hosting is a background service: the app-level auto-reconnect resumes
    // it on startup, the sync banner keeps reporting progress, and the peer
    // must still be able to rejoin while this window shows other pages.
    // Only the explicit Disconnect button ends a session (handleDisconnect).
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
  // Same as above: never kill the live session from UI lifecycle hooks —
  // navigating away from this component used to silently stop hosting and
  // strand the joiner on a dead room ("Rejoin does nothing").
  stopEventListeners();
});
</script>

<style scoped>
.connect-illustration {
  opacity: 0.9;
}
</style>
