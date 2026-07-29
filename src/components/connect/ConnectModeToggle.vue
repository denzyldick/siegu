<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { ConnectMode } from '@/composables/useConnect'

const props = defineProps<{
  modelValue: ConnectMode
}>()

const emit = defineEmits<{
  'update:modelValue': [value: ConnectMode]
}>()

const { t } = useI18n()

const modeDesc = computed(() => {
  return props.modelValue === 'host' ? t('connect.host_desc') : t('connect.join_desc')
})
</script>

<template>
  <div class="d-flex flex-column align-center mb-6 ga-3">
    <v-btn-toggle
      :model-value="modelValue"
      mandatory
      variant="flat"
      class="ga-2 bg-transparent"
      @update:model-value="emit('update:modelValue', $event)"
    >
      <v-btn value="host" class="siegu-btn text-none px-6">{{ $t('connect.host') }}</v-btn>
      <v-btn value="join" class="siegu-btn text-none px-6">{{ $t('connect.join') }}</v-btn>
    </v-btn-toggle>
    <div class="text-caption text-zinc-muted text-center px-4">{{ modeDesc }}</div>
  </div>
</template>
