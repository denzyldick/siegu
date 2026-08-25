<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border overflow-hidden">
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="surface" size="32" class="mr-3">
          <v-icon color="on-surface" size="small">mdi-signal-variant</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
        $t('settings.signalling')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-2">
      <v-list lines="two" class="bg-transparent">
        <v-list-item class="px-0">
          <template v-slot:title>
            <span class="font-weight-bold text-high-emphasis">{{
              $t('settings.signalling_url')
            }}</span>
          </template>
          <template v-slot:subtitle>
            <span class="text-medium-emphasis">{{ $t('settings.signalling_url_desc') }}</span>
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
        class="d-flex align-center pa-3 rounded-lg mb-4 border"
        :style="
          pingResult.ok
            ? 'background: rgba(var(--v-theme-success), 0.12)'
            : 'background: rgba(var(--v-theme-error), 0.12)'
        "
      >
        <v-icon size="small" class="mr-2" :color="pingResult.ok ? 'success' : 'error'">
          {{ pingResult.ok ? 'mdi-check-circle-outline' : 'mdi-alert-circle-outline' }}
        </v-icon>
        <span
          class="text-caption font-weight-bold"
          :style="
            pingResult.ok
              ? 'color: rgb(var(--v-theme-success))'
              : 'color: rgb(var(--v-theme-error))'
          "
        >
          {{ pingResult.message }}
        </span>
      </div>

      <div class="d-flex ga-2 mb-4">
        <v-btn
          size="small"
          variant="flat"
          color="primary"
          class="px-4"
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
          class="px-4"
          :loading="saving"
          :disabled="testing"
          @click="$emit('save')"
        >
          <v-icon start size="16">mdi-content-save-outline</v-icon>
          <span class="font-weight-bold">{{ $t('settings.signalling_save') }}</span>
        </v-btn>
      </div>

      <a
        href="https://github.com/denzyl/siegu/blob/main/docs/SIGNALLING.md"
        target="_blank"
        class="d-inline-flex align-center text-caption font-weight-medium mb-4"
        style="color: rgb(var(--v-theme-info)); text-decoration: none"
      >
        <v-icon size="14" class="mr-1">mdi-book-open-outline</v-icon>
        {{ $t('settings.signalling_docs') }} →
      </a>

      <v-alert
        variant="tonal"
        color="primary"
        rounded="lg"
        class="mb-2"
      >
        <div class="d-flex align-center">
          <v-icon size="20" class="mr-3">mdi-cloud-outline</v-icon>
          <div>
            <div class="text-body-2 font-weight-bold">{{ $t('settings.signalling_upsell_title') }}</div>
            <div class="text-caption text-medium-emphasis mb-1">{{ $t('settings.signalling_upsell_desc') }}</div>
            <div class="text-caption text-medium-emphasis" style="line-height: 1.5">
              {{ $t('settings.signalling_upsell_benefits') }}
            </div>
          </div>
        </div>
        <template v-slot:append>
          <v-btn
            size="small"
            variant="flat"
            color="primary"
            href="https://siegu.io/connect"
            target="_blank"
            class="text-none font-weight-bold"
          >
            {{ $t('settings.signalling_upsell_cta') }}
          </v-btn>
        </template>
      </v-alert>
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

<style scoped></style>
