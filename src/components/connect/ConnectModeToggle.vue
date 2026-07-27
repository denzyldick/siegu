<script setup lang="ts">
import type { ConnectMode } from '@/composables/useConnect'

defineProps<{
  modelValue: ConnectMode
}>()

const emit = defineEmits<{
  'update:modelValue': [value: ConnectMode]
}>()
</script>

<template>
  <div class="d-flex justify-center mb-6">
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
  </div>
</template>
