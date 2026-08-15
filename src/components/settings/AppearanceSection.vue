<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border overflow-hidden">
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="on-surface" size="32" class="mr-3">
          <v-icon color="surface" size="small">mdi-theme-light-dark</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
        $t('settings.appearance')
      }}</v-card-title>
    </v-card-item>
    <v-card-text class="pt-4">
      <v-radio-group
        :model-value="currentTheme"
        @update:model-value="onThemeChange"
        hide-details
        class="mt-0"
      >
        <v-radio value="system">
          <template v-slot:label>
            <span class="d-flex align-center">
              <v-icon size="small" color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2"
                >mdi-theme-light-dark</v-icon
              >
              {{ $t('settings.theme_system') }}
            </span>
          </template>
        </v-radio>
        <v-radio value="light">
          <template v-slot:label>
            <span class="d-flex align-center">
              <v-icon size="small" color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2"
                >mdi-white-balance-sunny</v-icon
              >
              {{ $t('settings.theme_light') }}
            </span>
          </template>
        </v-radio>
        <v-radio value="dark">
          <template v-slot:label>
            <span class="d-flex align-center">
              <v-icon size="small" color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2"
                >mdi-weather-night</v-icon
              >
              {{ $t('settings.theme_dark') }}
            </span>
          </template>
        </v-radio>
      </v-radio-group>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useTheme } from 'vuetify';
import { syncTheme } from '@/services/theme';

const props = defineProps<{
  initialTheme: string;
}>();

const theme = useTheme();
const currentTheme = ref(props.initialTheme);

function onThemeChange(val: string | null): void {
  if (!val) return;
  currentTheme.value = val;
  localStorage.setItem('siegu_theme', val);
  syncTheme((resolved) => {
    theme.global.name.value = resolved;
  });
}
</script>
