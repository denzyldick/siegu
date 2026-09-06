<template>
  <v-card
    variant="flat"
    color="surface"
    rounded="xl"
    class="mb-6 border overflow-hidden"
    data-tour="settings-map"
  >
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="surface" size="32" class="mr-3">
          <v-icon color="on-surface" size="small">mdi-map-outline</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
        $t('settings.map')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-2">
      <p class="text-body-2 text-medium-emphasis mb-4">{{ $t('settings.map_hint') }}</p>

      <v-text-field
        v-model="tileUrl"
        :label="$t('settings.map_tile_url')"
        :placeholder="defaultTileUrl"
        variant="outlined"
        density="comfortable"
        hide-details
        class="mb-4"
        :prepend-inner-icon="'mdi-web'"
      ></v-text-field>

      <v-text-field
        v-model="tileKey"
        :label="$t('settings.map_tile_key')"
        type="password"
        autocomplete="off"
        variant="outlined"
        density="comfortable"
        hide-details
        class="mb-4"
        :prepend-inner-icon="'mdi-key-outline'"
      ></v-text-field>

      <v-btn
        size="small"
        variant="flat"
        color="primary"
        class="px-4"
        :loading="saving"
        @click="save"
      >
        <v-icon start size="16">mdi-content-save-outline</v-icon>
        <span class="font-weight-bold">{{ $t('settings.save') }}</span>
      </v-btn>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { getConfig, saveConfig } from '@/services/tauri';

const defaultTileUrl =
  'https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}{r}.png?key=YOUR_KEY';
const tileUrl = ref('');
const tileKey = ref('');
const saving = ref(false);

async function load(): Promise<void> {
  try {
    const config = await getConfig();
    tileUrl.value = config['map_tile_url'] ?? '';
    tileKey.value = config['map_tile_key'] ?? '';
  } catch (e) {
    console.error('[MapSection] Failed to load:', e);
  }
}

async function save(): Promise<void> {
  saving.value = true;
  try {
    await saveConfig('map_tile_url', tileUrl.value.trim());
    await saveConfig('map_tile_key', tileKey.value.trim());
  } catch (e) {
    console.error('[MapSection] Failed to save:', e);
  } finally {
    saving.value = false;
  }
}

onMounted(load);
</script>