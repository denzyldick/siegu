<template>
  <v-card
    variant="flat"
    color="surface"
    rounded="xl"
    class="mb-6 border overflow-hidden"
    data-tour="settings-pro"
  >
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="surface" size="32" class="mr-3">
          <v-icon color="on-surface" size="small">mdi-crown-outline</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
        $t('settings.pro')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-2">
      <p class="text-body-2 text-medium-emphasis mb-4" style="line-height: 1.5">
        {{ $t('settings.pro_desc') }}
      </p>

      <v-text-field
        v-model="email"
        :label="$t('settings.pro_email')"
        type="email"
        variant="outlined"
        density="comfortable"
        hide-details
        class="mb-4"
        :prepend-inner-icon="'mdi-email-outline'"
      ></v-text-field>

      <v-expand-transition>
        <div v-if="result" class="mb-4">
          <div
            class="d-flex align-center pa-3 rounded-lg border"
            :style="
              result.verified
                ? 'background: rgba(var(--v-theme-success), 0.12)'
                : 'background: rgba(var(--v-theme-error), 0.12)'
            "
          >
            <v-icon size="small" class="mr-2" :color="result.verified ? 'success' : 'error'">
              {{ result.verified ? 'mdi-check-decagram' : 'mdi-alert-circle-outline' }}
            </v-icon>
            <span
              class="text-caption font-weight-bold"
              :style="
                result.verified
                  ? 'color: rgb(var(--v-theme-success))'
                  : 'color: rgb(var(--v-theme-error))'
              "
            >
              {{ resultText }}
            </span>
          </div>
        </div>
      </v-expand-transition>

      <div class="d-flex ga-2 mb-4 flex-wrap">
        <v-btn
          size="small"
          variant="flat"
          color="primary"
          class="px-4"
          :loading="sending"
          :disabled="verifying"
          @click="$emit('send')"
        >
          <v-icon start size="16">mdi-email-send-outline</v-icon>
          <span class="font-weight-bold">{{ $t('settings.pro_send') }}</span>
        </v-btn>
        <v-btn
          size="small"
          variant="flat"
          color="primary"
          class="px-4"
          :loading="verifying"
          :disabled="sending"
          @click="$emit('check')"
        >
          <v-icon start size="16">mdi-shield-check-outline</v-icon>
          <span class="font-weight-bold">{{ $t('settings.pro_check') }}</span>
        </v-btn>
      </div>

      <p class="text-caption text-medium-emphasis mb-4" style="line-height: 1.5">
        {{ $t('settings.pro_steps') }}
      </p>

      <v-divider class="my-3"></v-divider>

      <v-expand-transition>
        <div v-if="advanced">
          <v-text-field
            v-model="licenseUrl"
            :label="$t('settings.pro_license_url')"
            variant="outlined"
            density="comfortable"
            hide-details
            class="mb-3"
            :prepend-inner-icon="'mdi-web'"
          ></v-text-field>
          <v-text-field
            v-model="licenseToken"
            :label="$t('settings.pro_license_token')"
            variant="outlined"
            density="comfortable"
            hide-details
            class="mb-3"
            :prepend-inner-icon="'mdi-key-outline'"
          ></v-text-field>
          <v-btn
            size="small"
            variant="flat"
            color="primary"
            class="px-4 mb-3"
            :loading="saving"
            @click="$emit('save-config')"
          >
            <v-icon start size="16">mdi-content-save-outline</v-icon>
            <span class="font-weight-bold">{{ $t('settings.pro_save_config') }}</span>
          </v-btn>
        </div>
      </v-expand-transition>

      <v-btn
        size="small"
        variant="text"
        color="secondary"
        class="text-none px-2"
        @click="advanced = !advanced"
      >
        <v-icon size="16" class="mr-1">{{
          advanced ? 'mdi-chevron-up' : 'mdi-chevron-down'
        }}</v-icon>
        {{ advanced ? $t('settings.pro_advanced_hide') : $t('settings.pro_advanced') }}
      </v-btn>

      <v-alert variant="tonal" color="primary" rounded="lg" class="mt-3">
        <div class="d-flex align-center">
          <v-icon size="20" class="mr-3">mdi-cart-outline</v-icon>
          <div>
            <div class="text-body-2 font-weight-bold">{{ $t('settings.pro_upsell_title') }}</div>
            <div class="text-caption text-medium-emphasis">
              {{ $t('settings.pro_upsell_desc') }}
            </div>
          </div>
        </div>
        <template v-slot:append>
          <v-btn
            size="small"
            variant="flat"
            color="primary"
            :href="proUrl"
            target="_blank"
            class="text-none font-weight-bold"
          >
            {{ $t('settings.pro_upsell_cta') }}
          </v-btn>
        </template>
      </v-alert>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import type { ProStatus } from '@/types/settings';

const props = defineProps<{
  email: string;
  sending: boolean;
  verifying: boolean;
  saving: boolean;
  result: ProStatus | null;
  licenseUrl: string;
  licenseToken: string;
  proUrl: string;
}>();

const emit = defineEmits<{
  'update:email': [value: string];
  'update:licenseUrl': [value: string];
  'update:licenseToken': [value: string];
  send: [];
  check: [];
  'save-config': [];
}>();

const email = ref(props.email);
const licenseUrl = ref(props.licenseUrl);
const licenseToken = ref(props.licenseToken);
const advanced = ref(false);
const result = ref(props.result);

watch(
  () => props.email,
  (v) => {
    email.value = v;
  },
);
watch(
  () => props.licenseUrl,
  (v) => {
    licenseUrl.value = v;
  },
);
watch(
  () => props.licenseToken,
  (v) => {
    licenseToken.value = v;
  },
);
watch(
  () => props.result,
  (v) => {
    result.value = v;
  },
);
watch(email, (v) => emit('update:email', v));
watch(licenseUrl, (v) => emit('update:licenseUrl', v));
watch(licenseToken, (v) => emit('update:licenseToken', v));

const resultText = computed(() => {
  const r = result.value;
  if (!r) return '';
  if (r.verified) return 'Pro unlocked';
  if (r.error === 'not_paid') return 'This email has not been found as a Pro purchase.';
  if (r.paid) return 'Payment found — check your inbox and click the verification link.';
  if (r.error) return `Error: ${r.error}`;
  return 'Not verified yet.';
});
</script>

<style scoped></style>
