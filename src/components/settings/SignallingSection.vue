<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border-subtle overflow-hidden">
    <v-card-item class="bg-zinc-100 py-4">
      <template v-slot:prepend>
        <div class="siegu-icon-circle-dark mr-3">
          <v-icon color="var(--color-text-btn)" size="small">mdi-signal-variant</v-icon>
        </div>
      </template>
      <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
        $t('settings.signalling')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-2">
      <v-list lines="two" class="bg-transparent">
        <v-list-item class="px-0">
          <template v-slot:title>
            <span class="font-weight-bold text-zinc-primary">{{
              $t('settings.signalling_url')
            }}</span>
          </template>
          <template v-slot:subtitle>
            <span class="text-zinc-secondary">{{ $t('settings.signalling_url_desc') }}</span>
          </template>
        </v-list-item>
      </v-list>

      <v-text-field
        v-model="url"
        :label="$t('settings.signalling_url')"
        placeholder="wss://siegu.io/ws"
        variant="outlined"
        density="comfortable"
        hide-details
        class="mb-4"
        :prepend-inner-icon="'mdi-web'"
      ></v-text-field>

      <v-text-field
        v-model="token"
        :label="$t('settings.signalling_token')"
        :placeholder="$t('settings.signalling_token_placeholder')"
        variant="outlined"
        density="comfortable"
        hide-details
        class="mb-2"
        :prepend-inner-icon="'mdi-key-outline'"
      ></v-text-field>

      <div
        v-if="pingResult"
        class="d-flex align-center pa-3 rounded-lg mb-4 border-subtle"
        :class="pingResult.ok ? 'bg-green-50' : 'bg-red-50'"
      >
        <v-icon size="small" class="mr-2" :color="pingResult.ok ? 'success' : 'error'">
          {{ pingResult.ok ? 'mdi-check-circle-outline' : 'mdi-alert-circle-outline' }}
        </v-icon>
        <span
          class="text-caption font-weight-bold"
          :class="pingResult.ok ? 'text-success' : 'text-error'"
        >
          {{ pingResult.message }}
        </span>
      </div>

      <div class="d-flex ga-2">
        <v-btn
          size="small"
          variant="flat"
          color="primary"
          class="siegu-btn px-4"
          :loading="testing"
          :disabled="saving"
          @click="$emit('test')"
        >
          <v-icon start size="16">mdi-wifi-check</v-icon>
          <span class="font-weight-bold">{{ $t('settings.signalling_test') }}</span>
        </v-btn>
        <v-btn
          size="small"
          variant="flat"
          color="primary"
          class="siegu-btn px-4"
          :loading="saving"
          :disabled="testing"
          @click="$emit('save')"
        >
          <v-icon start size="16">mdi-content-save-outline</v-icon>
          <span class="font-weight-bold">{{ $t('settings.signalling_save') }}</span>
        </v-btn>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import type { PingResult } from '@/services/signalling';

const props = defineProps<{
  modelValue: string;
  token: string;
  testing: boolean;
  saving: boolean;
  pingResult: PingResult | null;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: string];
  'update:token': [value: string];
  test: [];
  save: [];
}>();

const url = ref(props.modelValue);
const token = ref(props.token);

watch(
  () => props.modelValue,
  (val) => {
    url.value = val;
  },
);
watch(
  () => props.token,
  (val) => {
    token.value = val;
  },
);
watch(url, (val) => emit('update:modelValue', val));
watch(token, (val) => emit('update:token', val));
</script>

<style scoped>
.bg-green-50 {
  background-color: var(--color-success-tint);
}
.bg-red-50 {
  background-color: var(--color-error-tint);
}
</style>
