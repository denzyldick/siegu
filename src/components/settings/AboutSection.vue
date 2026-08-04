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
          <v-icon color="#ffffff" size="small">mdi-information-outline</v-icon>
        </div>
      </template>
      <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
        $t('about_title')
      }}</v-card-title>
    </v-card-item>
    <v-card-text class="pt-4">
      <div class="d-flex align-center justify-space-between py-1">
        <span class="font-weight-bold text-zinc-primary">Siegu</span>
        <span v-if="version" class="text-caption text-zinc-secondary">v{{ version }}</span>
      </div>
      <v-divider class="my-3"></v-divider>
      <v-expansion-panels class="bg-transparent">
        <v-expansion-panel class="bg-transparent">
          <v-expansion-panel-title class="px-0 text-body-2 font-weight-bold text-zinc-primary">
            <v-icon size="small" color="#71717a" class="mr-2">mdi-xml</v-icon>
            {{ $t('about_open_source_licenses') }}
          </v-expansion-panel-title>
          <v-expansion-panel-text class="text-zinc-secondary">
            <div class="text-caption mb-2">{{ $t('about_licenses_intro') }}</div>
            <div
              v-for="dep in dependencies"
              :key="dep.name"
              class="d-flex justify-space-between py-1"
            >
              <span class="text-body-2 text-zinc-primary">{{ dep.name }}</span>
              <span class="text-caption text-zinc-secondary">{{ dep.license }}</span>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getVersion } from '@tauri-apps/api/app'

const version = ref('')

const dependencies = [
  { name: 'Vue.js', license: 'MIT' },
  { name: 'Vuetify', license: 'MIT' },
  { name: 'Vite', license: 'MIT' },
  { name: 'TypeScript', license: 'Apache-2.0' },
  { name: 'Tauri', license: 'MIT OR Apache-2.0' },
  { name: 'ONNX Runtime', license: 'MIT' },
  { name: 'Rust', license: 'MIT OR Apache-2.0' },
  { name: 'FFmpeg', license: 'LGPL-2.1+' },
]

onMounted(async () => {
  try {
    version.value = await getVersion()
  } catch {
    version.value = ''
  }
})
</script>
