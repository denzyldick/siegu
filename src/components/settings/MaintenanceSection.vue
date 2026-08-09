<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border-subtle overflow-hidden">
    <v-card-item class="bg-zinc-100 py-4">
      <template v-slot:prepend>
        <div class="siegu-icon-circle-dark mr-3">
          <v-icon color="var(--color-text-btn)" size="small">mdi-wrench-outline</v-icon>
        </div>
      </template>
      <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
        $t('settings.maintenance')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-2">
      <v-list lines="two" class="bg-transparent">
        <v-list-item class="px-0">
          <template v-slot:title>
            <span class="font-weight-bold text-zinc-primary">{{ $t('settings.cleanup_db') }}</span>
          </template>
          <template v-slot:subtitle>
            <span class="text-zinc-secondary">{{ $t('settings.cleanup_db_desc') }}</span>
          </template>
          <template v-slot:append>
            <v-btn
              size="small"
              variant="flat"
              color="primary"
              @click="cleanupDialog.show = true"
              :loading="isCleaning"
              class="siegu-btn px-4"
            >
              <v-icon start size="16">mdi-trash-can-outline</v-icon>
              <span class="font-weight-bold">{{ $t('settings.clean') }}</span>
            </v-btn>
          </template>
        </v-list-item>
      </v-list>

      <v-divider class="my-4 border-subtle"></v-divider>

      <div class="mb-6">
        <div class="text-caption font-weight-bold text-zinc-muted mb-4 tracking-widest uppercase">
          {{ $t('settings.advanced') }}
        </div>
        <div class="pt-2">
          <div class="d-flex justify-space-between align-center mb-2">
            <div class="text-caption font-weight-bold text-zinc-primary">
              {{ $t('settings.scan_threads') }}
            </div>
            <v-chip size="small" variant="flat" class="bg-btn font-weight-bold">{{
              performance.scanThreads
            }}</v-chip>
          </div>
          <v-slider
            :model-value="performance.scanThreads"
            :min="1"
            :max="maxThreads"
            :step="1"
            hide-details
            color="primary"
            track-color="var(--color-bg-zinc-100)"
            @update:model-value="onScanThreadsChange"
          ></v-slider>
        </div>
      </div>

      <v-divider class="my-6 border-subtle"></v-divider>

      <div>
        <div class="text-caption font-weight-bold text-zinc-muted mb-4 tracking-widest uppercase">
          {{ $t('settings.system_logs') }}
        </div>
        <v-sheet
          color="var(--color-bg-zinc-100)"
          class="pa-4 rounded-lg overflow-y-auto border-subtle debug-logs-sheet mb-4"
          max-height="300"
        >
          <div
            v-for="(log, i) in logs"
            :key="i"
            :class="log.type === 'error' ? 'text-error' : 'text-zinc-secondary'"
            class="mb-1"
            style="font-family: monospace; font-size: 11px"
          >
            <span class="text-zinc-muted">[{{ log.time }}]</span> {{ log.message }}
          </div>
          <div v-if="logs.length === 0" class="text-zinc-muted text-center py-4 text-caption">
            {{ $t('settings.no_logs') }}
          </div>
        </v-sheet>

        <div v-if="logs.length > 0" class="d-flex justify-center">
          <v-btn
            variant="text"
            size="small"
            class="text-none font-weight-bold"
            color="primary"
            prepend-icon="mdi-content-copy"
            @click.stop="copyLogs()"
          >
            {{ $t('settings.copy_logs') }}
          </v-btn>
          <v-btn
            variant="text"
            size="small"
            class="text-none font-weight-bold"
            color="error"
            prepend-icon="mdi-trash-can-outline"
            @click.stop="clearLogs()"
          >
            {{ $t('settings.clear_logs') }}
          </v-btn>
        </div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { storeToRefs } from 'pinia';
import { useI18n } from 'vue-i18n';
import { useSettingsStore } from '@/stores/settings';

const store = useSettingsStore();
const { t } = useI18n();

const { isCleaning, logs } = storeToRefs(store);

const { performance, maxThreads, cleanupDialog, setScanThreads, clearLogs, showSnackbar } = store;

function onScanThreadsChange(value: number | [number, number]): void {
  void setScanThreads(typeof value === 'number' ? value : value[0]);
}

async function copyLogs(): Promise<void> {
  try {
    const text = logs.value.map((log) => `[${log.time}] ${log.message}`).join('\n');
    await navigator.clipboard.writeText(text);
    showSnackbar(t('settings.logs_copied'));
  } catch {
    showSnackbar(t('settings.logs_copy_failed'), true);
  }
}
</script>

<style scoped>
.debug-logs-sheet {
  max-height: 300px;
}
</style>
