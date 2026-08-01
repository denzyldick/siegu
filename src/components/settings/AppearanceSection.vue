<template>
  <v-card
    variant="flat"
    color="surface"
    rounded="xl"
    class="mb-6 border-subtle overflow-hidden"
  >
    <v-card-item class="bg-zinc-100 py-4">
      <template v-slot:prepend>
        <div class="siegu-icon-circle-dark mr-3">
          <v-icon color="#ffffff" size="small">mdi-theme-light-dark</v-icon>
        </div>
      </template>
      <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
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
        <v-radio :label="$t('settings.theme_system')" value="system">
          <template v-slot:prepend>
            <v-icon size="small" color="#71717a" class="mr-2">mdi-theme-light-dark</v-icon>
          </template>
        </v-radio>
        <v-radio :label="$t('settings.theme_light')" value="light">
          <template v-slot:prepend>
            <v-icon size="small" color="#f59e0b" class="mr-2">mdi-white-balance-sunny</v-icon>
          </template>
        </v-radio>
        <v-radio :label="$t('settings.theme_dark')" value="dark">
          <template v-slot:prepend>
            <v-icon size="small" color="#71717a" class="mr-2">mdi-weather-night</v-icon>
          </template>
        </v-radio>
      </v-radio-group>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useTheme } from 'vuetify'

const props = defineProps<{
  initialTheme: string
}>()

const theme = useTheme()
const currentTheme = ref(props.initialTheme)

function onThemeChange(val: string | null): void {
  if (!val) return
  currentTheme.value = val
  localStorage.setItem('siegu_theme', val)

  if (val === 'system') {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    theme.global.name.value = prefersDark ? 'dark' : 'light'
    document.documentElement.dataset.theme = prefersDark ? 'dark' : 'light'
  } else {
    theme.global.name.value = val
    document.documentElement.dataset.theme = val
  }
}
</script>
