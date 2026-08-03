<script setup lang="ts">
defineProps<{
  status: string;
  isConnected: boolean;
  showDisconnect: boolean;
  disconnecting: boolean;
}>();

const emit = defineEmits<{
  disconnect: [];
}>();
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

  <div v-if="showDisconnect" class="text-center mt-4">
    <v-btn
      variant="flat"
      color="primary"
      size="small"
      @click="emit('disconnect')"
      :loading="disconnecting"
      prepend-icon="mdi-close"
    >
      {{ $t('devices.disconnect') }}
    </v-btn>
  </div>
</template>
