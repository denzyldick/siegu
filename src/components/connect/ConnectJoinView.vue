<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';

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

const syncStatusText = computed(() => {
  if (props.itemsTotal > 0) {
    return t('connect.syncing_files', {
      completed: props.itemsCompleted,
      total: props.itemsTotal,
    });
  }
  return t('connect.syncing');
});
</script>

<template>
  <div v-if="hostIp" class="d-flex flex-column align-stretch mb-6 ga-4" style="width: 100%">
    <div class="text-center mb-2">
      <div class="text-caption text-zinc-muted">Direct connection</div>
      <div class="text-body-1 font-weight-bold text-zinc-primary">{{ hostIp }}:{{ hostPort }}</div>
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
      class="text-center join-passphrase"
      style="width: 100%; min-width: 280px"
      @keyup.enter="emit('join', hostIp, String(hostPort))"
      :disabled="loading"
    ></v-text-field>

    <v-btn
      v-if="!showSyncButton && !isConnected"
      variant="flat"
      color="black"
      @click="emit('join', hostIp, String(hostPort))"
      class="siegu-btn py-6"
      block
      :disabled="loading || !modelValue"
    >
      <div v-if="loading" class="d-flex align-center">
        <v-progress-circular indeterminate size="18" width="2" color="white" class="mr-3" />
        <span>{{ t('connect.joining') }}</span>
      </div>
      <div v-else class="d-flex align-center">
        <div class="siegu-icon-circle mr-3">
          <v-icon size="14">mdi-link-variant</v-icon>
        </div>
        <span>{{ t('connect.link_device_button') }}</span>
      </div>
    </v-btn>

    <div v-if="syncing" class="d-flex flex-column align-center py-4 ga-2">
      <div v-if="deviceName" class="text-caption text-zinc-secondary">
        {{ t('connect.connected_to') }} <strong>{{ deviceName }}</strong>
      </div>
      <div v-if="itemsTotal > 0" class="d-flex align-center ga-2 w-100">
        <v-progress-linear
          :model-value="(itemsCompleted / itemsTotal) * 100"
          color="success"
          height="6"
          rounded
          class="flex-grow-1"
        />
        <span class="text-caption text-success font-weight-bold">
          {{ itemsCompleted }}/{{ itemsTotal }}
        </span>
      </div>
      <v-progress-linear v-else indeterminate color="success" height="4" class="w-100" />
      <span class="text-caption text-zinc-muted">{{ syncStatusText }}</span>
    </div>

    <div v-else-if="isConnected" class="d-flex flex-column align-center py-4 ga-1">
      <v-icon size="16" color="success" class="mr-2">mdi-check-circle</v-icon>
      <span class="text-caption text-success">{{ t('connect.sync_complete') }}</span>
      <span v-if="itemsCompleted > 0" class="text-caption text-zinc-muted">
        {{ itemsCompleted }} {{ t('connect.files_synced') }}
      </span>
      <span v-else class="text-caption text-zinc-muted">{{ t('connect.all_up_to_date') }}</span>
    </div>
  </div>
</template>

<style scoped>
.join-passphrase :deep(.v-field) {
  background: #a1a1aa !important;
}

.join-passphrase :deep(.v-field__overlay) {
  background: #a1a1aa !important;
}

.join-passphrase :deep(.v-field--focused),
.join-passphrase :deep(.v-field--focused .v-field__overlay) {
  background: #18181b !important;
}

.join-passphrase :deep(input) {
  color: #fafafa !important;
  caret-color: #fafafa !important;
  font-size: 14px !important;
  letter-spacing: 0.02em;
}

.join-passphrase :deep(.v-field__input) {
  color: #fafafa !important;
  min-height: 40px !important;
  padding: 4px 8px !important;
}

.join-passphrase :deep(.v-label) {
  color: var(--color-text-muted) !important;
}
</style>
