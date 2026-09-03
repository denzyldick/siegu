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
          :loading="sending || verifying"
          @click="$emit('verify')"
        >
          <v-icon start size="16">mdi-shield-check-outline</v-icon>
          <span class="font-weight-bold">{{ $t('settings.pro_verify') }}</span>
        </v-btn>
      </div>

      <p class="text-caption text-medium-emphasis mb-4" style="line-height: 1.5">
        {{ $t('settings.pro_steps_short') }}
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
          <v-btn
            size="small"
            variant="flat"
            color="primary"
            class="px-4 mb-4"
            :loading="saving"
            @click="$emit('save-config')"
          >
            <v-icon start size="16">mdi-content-save-outline</v-icon>
            <span class="font-weight-bold">{{ $t('settings.pro_save_config') }}</span>
          </v-btn>

          <v-divider class="mb-4"></v-divider>

          <div class="d-flex align-center mb-1">
            <v-icon size="16" class="mr-2" color="on-surface">mdi-signal-variant</v-icon>
            <span class="text-subtitle-2 font-weight-bold text-high-emphasis">{{
              $t('settings.signalling')
            }}</span>
          </div>
          <p class="text-caption text-medium-emphasis mb-3" style="line-height: 1.5">
            {{ $t('settings.signalling_url_desc') }}
          </p>

          <v-text-field
            v-model="signallingUrl"
            :label="$t('settings.signalling_url')"
            :placeholder="defaultSignallingUrl"
            variant="outlined"
            density="comfortable"
            hide-details
            class="mb-3"
            :prepend-inner-icon="'mdi-web'"
          ></v-text-field>

          <v-text-field
            v-model="signallingToken"
            :label="$t('settings.signalling_token')"
            :placeholder="$t('settings.signalling_token_placeholder')"
            variant="outlined"
            density="comfortable"
            hide-details
            class="mb-2"
            :prepend-inner-icon="'mdi-key-outline'"
          ></v-text-field>

          <div
            v-if="signalPingResult"
            class="d-flex align-center pa-3 rounded-lg mb-4 border"
            :style="
              signalPingResult.ok
                ? 'background: rgba(var(--v-theme-success), 0.12)'
                : 'background: rgba(var(--v-theme-error), 0.12)'
            "
          >
            <v-icon size="small" class="mr-2" :color="signalPingResult.ok ? 'success' : 'error'">
              {{ signalPingResult.ok ? 'mdi-check-circle-outline' : 'mdi-alert-circle-outline' }}
            </v-icon>
            <span
              class="text-caption font-weight-bold"
              :style="
                signalPingResult.ok
                  ? 'color: rgb(var(--v-theme-success))'
                  : 'color: rgb(var(--v-theme-error))'
              "
            >
              {{ signalPingResult.message }}
            </span>
          </div>

          <div class="d-flex ga-2 mb-4 flex-wrap">
            <v-btn
              size="small"
              variant="flat"
              color="primary"
              class="px-4"
              :loading="signallingTesting"
              :disabled="signallingSaving"
              @click="$emit('test-signalling')"
            >
              <v-icon start size="16">mdi-wifi-check</v-icon>
              <span class="font-weight-bold">{{ $t('settings.signalling_test') }}</span>
            </v-btn>
            <v-btn
              size="small"
              variant="flat"
              color="primary"
              class="px-4"
              :loading="signallingSaving"
              :disabled="signallingTesting"
              @click="$emit('save-signalling')"
            >
              <v-icon start size="16">mdi-content-save-outline</v-icon>
              <span class="font-weight-bold">{{ $t('settings.signalling_save') }}</span>
            </v-btn>
          </div>

          <a
            href="https://github.com/denzyldick/siegu/blob/main/docs/SIGNALLING.md"
            target="_blank"
            class="d-inline-flex align-center text-caption font-weight-medium mb-4"
            style="color: rgb(var(--v-theme-info)); text-decoration: none"
          >
            <v-icon size="14" class="mr-1">mdi-book-open-outline</v-icon>
            {{ $t('settings.signalling_docs') }} →
          </a>

          <v-alert variant="tonal" color="primary" rounded="lg" class="mb-2">
            <div class="d-flex align-center">
              <v-icon size="20" class="mr-3">mdi-cloud-outline</v-icon>
              <div>
                <div class="text-body-2 font-weight-bold">
                  {{ $t('settings.signalling_upsell_title') }}
                </div>
                <div class="text-caption text-medium-emphasis mb-1">
                  {{ $t('settings.signalling_upsell_desc') }}
                </div>
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
                :href="connectUrl"
                target="_blank"
                class="text-none font-weight-bold"
              >
                {{ $t('settings.signalling_upsell_cta') }}
              </v-btn>
            </template>
          </v-alert>
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

      <v-dialog v-model="dialogOpen" max-width="420" persistent>
        <v-card variant="flat" color="surface" rounded="xl" class="pa-4">
          <v-card-text>
            <div v-if="verifyDialogSending" class="text-center py-4">
              <v-progress-circular
                indeterminate
                color="primary"
                size="40"
                class="mb-4"
              ></v-progress-circular>
              <div class="text-body-2 font-weight-bold">
                {{ $t('settings.pro_dialog_sending') }}
              </div>
            </div>
            <div v-else class="text-center py-4">
              <v-icon size="44" class="mb-3" :color="result?.verified ? 'success' : 'primary'">
                {{ result?.verified ? 'mdi-check-decagram' : 'mdi-email-fast-outline' }}
              </v-icon>
              <div class="text-body-2 font-weight-bold mb-1">
                {{ $t('settings.pro_dialog_waiting') }}
              </div>
              <p class="text-caption text-medium-emphasis" style="line-height: 1.5">
                {{ $t('settings.pro_dialog_waiting_desc') }} <strong>{{ email }}</strong>
              </p>
              <div
                v-if="verifyDialogVerifying"
                class="d-flex align-center justify-center ga-2 mt-3"
              >
                <v-progress-circular indeterminate size="16" color="primary"></v-progress-circular>
                <span class="text-caption">{{ $t('settings.pro_dialog_watching') }}</span>
              </div>
            </div>
          </v-card-text>
          <v-card-actions class="justify-end">
            <v-btn variant="text" color="secondary" @click="$emit('close-verify-dialog')">
              {{ $t('settings.pro_dialog_close') }}
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-dialog>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import type { ProStatus } from '@/types/settings';
import type { PingResult } from '@/services/signalling';
import { DEFAULT_SIGNALING_URL, APP_CONNECT_URL } from '@/services/appConfig';

const props = defineProps<{
  email: string;
  sending: boolean;
  verifying: boolean;
  saving: boolean;
  result: ProStatus | null;
  licenseUrl: string;
  proUrl: string;
  signallingUrl: string;
  signallingToken: string;
  signallingTesting: boolean;
  signallingSaving: boolean;
  signalPingResult: PingResult | null;
  connectUrl?: string;
  verifyDialog: boolean;
  verifyDialogSending: boolean;
  verifyDialogVerifying: boolean;
}>();

const emit = defineEmits<{
  'update:email': [value: string];
  'update:licenseUrl': [value: string];
  'update:signallingUrl': [value: string];
  'update:signallingToken': [value: string];
  verify: [];
  'close-verify-dialog': [];
  'save-config': [];
  'test-signalling': [];
  'save-signalling': [];
}>();

const email = ref(props.email);
const licenseUrl = ref(props.licenseUrl);
const advanced = ref(false);
const result = ref(props.result);
const signallingUrl = ref(props.signallingUrl);
const signallingToken = ref(props.signallingToken);

const defaultSignallingUrl = computed(() => DEFAULT_SIGNALING_URL);
const connectUrl = computed(() => props.connectUrl || APP_CONNECT_URL);
const dialogOpen = computed({
  get: () => props.verifyDialog,
  set: (v: boolean) => {
    if (!v) emit('close-verify-dialog');
  },
});

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
  () => props.result,
  (v) => {
    result.value = v;
  },
);
watch(
  () => props.signallingUrl,
  (v) => {
    signallingUrl.value = v;
  },
);
watch(
  () => props.signallingToken,
  (v) => {
    signallingToken.value = v;
  },
);
watch(email, (v) => emit('update:email', v));
watch(licenseUrl, (v) => emit('update:licenseUrl', v));
watch(signallingUrl, (v) => emit('update:signallingUrl', v));
watch(signallingToken, (v) => emit('update:signallingToken', v));

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
