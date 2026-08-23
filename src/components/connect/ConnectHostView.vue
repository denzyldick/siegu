<template>
  <div class="d-flex justify-center ga-5 mb-4">
    <v-icon size="28" color="rgba(var(--v-theme-on-surface), 0.7)">
      {{ isConnected && peerOs ? deviceOsIcon(peerOs) : 'mdi-laptop' }}
    </v-icon>
    <template v-if="!isConnected">
      <v-icon size="28" color="rgba(var(--v-theme-on-surface), 0.7)">mdi-microsoft-windows</v-icon>
      <v-icon size="28" color="rgba(var(--v-theme-on-surface), 0.7)">mdi-apple</v-icon>
      <v-icon size="28" color="rgba(var(--v-theme-on-surface), 0.7)">mdi-linux</v-icon>
      <v-icon size="28" color="rgba(var(--v-theme-on-surface), 0.7)">mdi-android</v-icon>
      <v-icon size="28" color="rgba(var(--v-theme-on-surface), 0.7)">mdi-apple-ios</v-icon>
    </template>
  </div>

  <template v-if="isConnected">
    <div class="d-flex flex-column align-center py-2 ga-2" style="width: 100%">
      <div v-if="peerName" class="d-flex align-center ga-2">
        <v-icon color="success" size="18">mdi-check-circle</v-icon>
        <span class="text-body-2 font-weight-bold" style="color: rgb(var(--v-theme-success))">
          {{ $t('connect.connected_to') }} {{ peerName }}
        </span>
        <v-icon size="16" class="text-medium-emphasis">{{ deviceOsIcon(peerOs) }}</v-icon>
      </div>
      <div v-if="syncing" class="w-100">
        <SyncNowCard
          :progress="progress"
          :items-completed="itemsCompleted"
          :items-total="itemsTotal"
        />
      </div>
    </div>
  </template>

  <template v-else>
    <div class="text-caption text-disabled text-center mb-4 px-2" style="max-width: 320px">
      {{ $t('connect.host_instructions') }}
    </div>

    <div class="d-flex justify-center flex-wrap gap-2 mb-3" v-if="passphrase.length > 0">
      <v-chip
        v-for="(word, index) in passphrase"
        :key="index"
        variant="outlined"
        class="font-weight-medium mx-1 text-high-emphasis"
        size="small"
      >
        {{ word }}
      </v-chip>
    </div>

    <v-btn
      v-if="!copied"
      variant="outlined"
      color="primary"
      size="small"
      class="text-none text-caption"
      prepend-icon="mdi-content-copy"
      @click="copyPassphrase"
    >
      {{ $t('connect.copy_phrase') }}
    </v-btn>
    <v-btn
      v-else
      variant="outlined"
      color="green"
      size="small"
      class="text-none text-caption"
      prepend-icon="mdi-check"
    >
      {{ $t('connect.copied') }}
    </v-btn>
  </template>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import SyncNowCard from '@/components/connect/SyncNowCard.vue';
import { deviceOsIcon } from '@/utils/format';

const props = withDefaults(
  defineProps<{
    passphrase: string[];
    isConnected?: boolean;
    syncing?: boolean;
    progress?: number;
    itemsCompleted?: number;
    itemsTotal?: number;
    peerName?: string;
    peerOs?: string;
  }>(),
  {
    isConnected: false,
    syncing: false,
    progress: 0,
    itemsCompleted: 0,
    itemsTotal: 0,
    peerName: '',
    peerOs: '',
  },
);

const copied = ref(false);

function copyPassphrase() {
  const text = props.passphrase.join(' ');
  navigator.clipboard.writeText(text);
  copied.value = true;
  setTimeout(() => {
    copied.value = false;
  }, 2000);
}
</script>
