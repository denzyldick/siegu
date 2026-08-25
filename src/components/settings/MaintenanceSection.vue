<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border overflow-hidden">
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="surface" size="32" class="mr-3">
          <v-icon color="on-surface" size="small">mdi-wrench-outline</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
        $t('settings.maintenance')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-2">
      <v-list lines="two" class="bg-transparent">
        <v-list-item class="px-0">
          <template v-slot:title>
            <span class="font-weight-bold text-high-emphasis">{{ $t('settings.cleanup_db') }}</span>
          </template>
          <template v-slot:subtitle>
            <span class="text-medium-emphasis">{{ $t('settings.cleanup_db_desc') }}</span>
          </template>
          <template v-slot:append>
            <v-btn
              size="small"
              variant="flat"
              color="error"
              @click="cleanupDialog.show = true"
              :loading="isCleaning"
              class="px-4"
            >
              <v-icon start size="16">mdi-trash-can-outline</v-icon>
              <span class="font-weight-bold">{{ $t('settings.clean') }}</span>
            </v-btn>
          </template>
        </v-list-item>
      </v-list>

      <v-divider class="my-4 border"></v-divider>

      <div>
        <div class="text-caption font-weight-bold text-disabled mb-4 tracking-widest uppercase">
          {{ $t('settings.system_logs') }}
        </div>
        <v-sheet
          class="pa-4 rounded-lg overflow-y-auto border debug-logs-sheet mb-4"
          max-height="300"
        >
          <div
            v-for="(log, i) in logs"
            :key="i"
            :style="
              log.type === 'error'
                ? 'color: rgb(var(--v-theme-error))'
                : 'color: rgba(var(--v-theme-on-surface), 0.7)'
            "
            class="mb-1"
            style="font-family: monospace; font-size: 11px"
          >
            <span class="text-disabled">[{{ log.time }}]</span> {{ log.message }}
          </div>
          <div v-if="logs.length === 0" class="text-disabled text-center py-4 text-caption">
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

const { cleanupDialog, clearLogs, showSnackbar } = store;

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
