<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border overflow-hidden">
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="on-surface" size="32" class="mr-3">
          <v-icon color="surface" size="small">mdi-database-outline</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
        $t('settings.storage')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-2">
      <v-list lines="two" class="bg-transparent">
        <v-list-item class="px-0">
          <template v-slot:title>
            <span class="font-weight-bold text-high-emphasis">{{
              $t('settings.storage_cap')
            }}</span>
          </template>
          <template v-slot:subtitle>
            <span class="text-medium-emphasis">{{ $t('settings.storage_cap_desc') }}</span>
          </template>
        </v-list-item>
      </v-list>

      <v-text-field
        v-model.number="capMb"
        type="number"
        min="1"
        max="1000000"
        :label="$t('settings.storage_cap_mb')"
        variant="outlined"
        density="comfortable"
        hide-details
        class="mb-4"
        :prepend-inner-icon="'mdi-harddisk'"
      ></v-text-field>

      <div v-if="usage" class="mb-4">
        <div class="d-flex justify-space-between mb-1">
          <span class="text-caption text-medium-emphasis">{{ $t('settings.storage_used') }}</span>
          <span class="text-caption font-weight-bold">
            {{ formatBytes(usage.used) }} / {{ formatBytes(usage.quota) }}
          </span>
        </div>
        <v-progress-linear
          :model-value="usagePercent"
          rounded
          height="8"
          :color="usagePercent > 90 ? 'error' : usagePercent > 75 ? 'warning' : 'primary'"
        ></v-progress-linear>
      </div>

      <v-btn
        size="small"
        variant="flat"
        color="primary"
        class="px-4"
        :loading="saving"
        :disabled="!valid"
        @click="save"
      >
        <v-icon start size="16">mdi-content-save-outline</v-icon>
        <span class="font-weight-bold">{{ $t('settings.signalling_save') }}</span>
      </v-btn>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { getConfig, saveConfig, getStorageUsage, type StorageUsage } from '@/services/tauri';

const capMb = ref<number>(10240);
const saving = ref(false);
const usage = ref<StorageUsage | null>(null);

const valid = computed(() => Number.isFinite(capMb.value) && capMb.value >= 1);

const usagePercent = computed(() => {
  if (!usage.value || !usage.value.quota) return 0;
  return Math.min(100, (usage.value.used / usage.value.quota) * 100);
});

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }
  return `${Math.round(bytes / (1024 * 1024))} MB`;
}

async function load(): Promise<void> {
  try {
    const config = await getConfig();
    const parsed = parseInt(config['max_storage_mb'] ?? '', 10);
    if (Number.isFinite(parsed) && parsed > 0) capMb.value = parsed;
    usage.value = await getStorageUsage();
  } catch (e) {
    console.error('[StorageSection] Failed to load:', e);
  }
}

async function save(): Promise<void> {
  saving.value = true;
  try {
    await saveConfig('max_storage_mb', String(Math.floor(capMb.value)));
  } catch (e) {
    console.error('[StorageSection] Failed to save:', e);
  } finally {
    saving.value = false;
  }
}

onMounted(load);
</script>

<style scoped></style>
