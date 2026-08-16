<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import SyncNowCard from '@/components/connect/SyncNowCard.vue';

const { t } = useI18n();

const props = defineProps<{
  modelValue: string;
  loading: boolean;
  isConnected: boolean;
  showSyncButton: boolean;
  syncing: boolean;
  hostIp: string;
  hostPort: number;
  deviceName: string;
  itemsCompleted: number;
  itemsTotal: number;
  progress: number;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: string];
  join: [ip: string, port: string];
  sync: [];
}>();

const localValue = computed({
  get: () => props.modelValue,
  set: (val: string) => emit('update:modelValue', val),
});
</script>

<template>
  <div v-if="hostIp" class="d-flex flex-column align-stretch mb-6 ga-4" style="width: 100%">
    <div v-if="deviceName" class="text-center mb-2">
      <div class="text-caption text-disabled">{{ t('connect.selected_device') }}</div>
      <div class="text-body-1 font-weight-bold text-high-emphasis">{{ deviceName }}</div>
    </div>

    <v-text-field
      v-if="!isConnected"
      v-model="localValue"
      :placeholder="t('connect.phrase_placeholder')"
      variant="solo-filled"
      density="comfortable"
      hide-details
      flat
      rounded="lg"
      class="text-center"
      style="width: 100%; min-width: 280px"
      @keyup.enter="emit('join', hostIp, String(hostPort))"
      :disabled="loading"
    ></v-text-field>

    <v-btn
      v-if="!showSyncButton && !isConnected"
      variant="flat"
      color="primary"
      @click="emit('join', hostIp, String(hostPort))"
      class="py-6"
      block
      :disabled="loading || !modelValue"
    >
      <div v-if="loading" class="d-flex align-center">
        <v-progress-circular indeterminate size="18" width="2" color="white" class="mr-3" />
        <span>{{ t('connect.joining') }}</span>
      </div>
      <div v-else class="d-flex align-center">
        <v-avatar color="rgba(255,255,255,0.2)" size="28" class="mr-3">
          <v-icon size="14">mdi-link-variant</v-icon>
        </v-avatar>
        <span>{{ t('connect.link_device_button') }}</span>
      </div>
    </v-btn>

    <div v-if="syncing" class="d-flex flex-column align-center py-4 ga-3">
      <SyncNowCard
        :progress="progress"
        :items-completed="itemsCompleted"
        :items-total="itemsTotal"
      />
    </div>

    <div v-else-if="isConnected" class="d-flex flex-column align-center py-4 ga-1">
      <v-icon size="16" color="success" class="mr-2">mdi-check-circle</v-icon>
      <span style="color: rgb(var(--v-theme-success))">{{ t('connect.sync_complete') }}</span>
      <span v-if="itemsCompleted > 0" class="text-caption text-disabled">
        {{ itemsCompleted }} {{ t('connect.files_synced') }}
      </span>
      <span v-else class="text-caption text-disabled">{{ t('connect.all_up_to_date') }}</span>
    </div>
  </div>
</template>
