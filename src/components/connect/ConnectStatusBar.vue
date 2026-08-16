<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { connectionStatusKey } from '@/utils/connectStatus';

const { t } = useI18n();

const props = defineProps<{
  status: string;
  isConnected: boolean;
  showDisconnect: boolean;
  disconnecting: boolean;
}>();

const displayStatus = computed(() => {
  const key = connectionStatusKey(props.status);
  return key ? t(key) : props.status;
});

const emit = defineEmits<{
  disconnect: [];
}>();
</script>

<template>
  <div class="text-caption text-disabled mb-1 text-center py-2" v-if="status && !isConnected">
    <v-progress-circular
      v-if="!isConnected && status !== $t('connect.disconnected')"
      indeterminate
      color="rgba(var(--v-theme-on-surface), 0.7)"
      size="16"
      width="2"
      class="mr-2 opacity-50"
    ></v-progress-circular>
    <v-icon v-else-if="isConnected" color="success" size="16" class="mr-2"
      >mdi-check-circle-outline</v-icon
    >
    {{ displayStatus }}
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
