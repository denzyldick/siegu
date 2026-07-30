<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

const props = defineProps<{
  modelValue: string
  loading: boolean
  isConnected: boolean
  showSyncButton: boolean
  syncing: boolean
  hostIp: string
  hostPort: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  join: [ip: string, port: string]
  sync: []
}>()

const localValue = computed({
  get: () => props.modelValue,
  set: (val: string) => emit('update:modelValue', val),
})
</script>

<template>
  <div v-if="hostIp" class="d-flex justify-center mb-6 flex-column ga-4">
    <div class="text-center mb-2">
      <div class="text-caption text-zinc-muted">Connect to</div>
      <div class="text-body-1 font-weight-bold text-zinc-primary">{{ hostIp }}:{{ hostPort }}</div>
    </div>

    <v-text-field
      v-model="localValue"
      :placeholder="t('connect.phrase_placeholder')"
      variant="solo-filled"
      density="comfortable"
      hide-details
      flat
      rounded="lg"
      class="text-center siegu-field join-passphrase"
      @keyup.enter="emit('join', hostIp, String(hostPort))"
      :disabled="loading || isConnected"
    ></v-text-field>

    <v-btn
      v-if="!showSyncButton"
      variant="flat"
      @click="emit('join', hostIp, String(hostPort))"
      class="siegu-btn py-6"
      block
      :loading="loading"
      :disabled="!modelValue || isConnected"
    >
      <div class="d-flex align-center">
        <div class="siegu-icon-circle mr-3">
          <v-icon size="14">mdi-link-variant</v-icon>
        </div>
        <span>{{ t('connect.link_device_button') }}</span>
      </div>
    </v-btn>

    <div
      v-else-if="syncing"
      class="d-flex align-center justify-center py-4"
    >
      <v-progress-linear
        indeterminate
        color="success"
        height="4"
        class="w-100"
      />
      <span class="text-caption text-success ml-3">{{ t('connect.syncing') }}</span>
    </div>

    <div
      v-else-if="isConnected"
      class="d-flex align-center justify-center py-4"
    >
      <v-icon size="16" color="success" class="mr-2">mdi-check-circle</v-icon>
      <span class="text-caption text-success">{{ t('connect.sync_complete') }}</span>
    </div>
  </div>
</template>

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

.join-passphrase :deep(input) {
  color: #fafafa !important;
  caret-color: #fafafa !important;
}

.join-passphrase :deep(.v-field__input) {
  color: #fafafa !important;
}

.join-passphrase :deep(.v-label) {
  color: #71717a !important;
}

.siegu-btn:deep(.v-btn--loading .v-btn__content) {
  opacity: 1;
}
</style>