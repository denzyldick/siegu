<script setup lang="ts">
import type { SyncProgressState } from '@/composables/useConnect'

defineProps<{
  status: string
  isConnected: boolean
  syncProgress: SyncProgressState
  showDisconnect: boolean
  disconnecting: boolean
}>()

const emit = defineEmits<{
  disconnect: []
}>()
</script>

<template>
  <div class="text-caption text-zinc-muted mb-1 text-center py-2" v-if="status">
    <v-progress-circular
      v-if="!isConnected && status !== $t('connect.disconnected')"
      indeterminate
      color="black"
      size="16"
      width="2"
      class="mr-2 opacity-50"
    ></v-progress-circular>
    <v-icon v-else-if="isConnected" color="success" size="16" class="mr-2"
      >mdi-check-circle-outline</v-icon
    >
    {{ status }}
  </div>

  <div v-if="syncProgress.status" class="mt-4 px-4">
    <div class="d-flex justify-space-between text-caption text-zinc-secondary mb-1">
      <span>{{ syncProgress.status }}</span>
      <span v-if="syncProgress.progress > 0">{{ Math.round(syncProgress.progress) }}%</span>
    </div>
    <v-progress-linear
      v-if="syncProgress.progress === 0 && syncProgress.status.includes('Syncing')"
      :model-value="0"
      color="black"
      height="6"
      rounded
      indeterminate
    ></v-progress-linear>
    <v-progress-linear
      v-else
      :model-value="syncProgress.progress"
      color="black"
      height="6"
      rounded
    ></v-progress-linear>
  </div>

  <div v-if="showDisconnect" class="text-center mt-4">
    <v-btn
      variant="flat"
      color="black"
      size="small"
      @click="emit('disconnect')"
      :loading="disconnecting"
      prepend-icon="mdi-close"
    >
      {{ $t('devices.disconnect') }}
    </v-btn>
  </div>
</template>
