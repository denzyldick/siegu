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
          <v-icon color="#ffffff" size="small">mdi-wrench-outline</v-icon>
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
            <span class="font-weight-bold text-zinc-primary">{{
              $t('settings.cleanup_db')
            }}</span>
          </template>
          <template v-slot:subtitle>
            <span class="text-zinc-secondary">{{ $t('settings.cleanup_db_desc') }}</span>
          </template>
          <template v-slot:append>
            <v-btn
              size="small"
              variant="flat"
              color="primary"
              @click="$emit('cleanup-db')"
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
            <v-chip
              size="small"
              color="#000000"
              variant="flat"
              class="font-weight-bold text-white"
              >{{ performance.scanThreads }}</v-chip
            >
          </div>
          <v-slider
            v-model="performance.scanThreads"
            :min="1"
            :max="maxThreads"
            :step="1"
            hide-details
            color="primary"
            track-color="#f4f4f5"
            @update:model-value="$emit('save-performance')"
          ></v-slider>

          <v-list-item class="px-0 mt-4">
            <v-list-item-title class="text-caption font-weight-bold text-zinc-primary">{{
              $t('settings.indexing_mode')
            }}</v-list-item-title>
            <template v-slot:append>
              <v-menu offset-y>
                <template v-slot:activator="{ props }">
                  <v-btn
                    variant="tonal"
                    size="small"
                    color="primary"
                    v-bind="props"
                    class="font-weight-bold"
                  >
                    {{ getModeLabel(performance.indexingMode) }}
                    <v-icon size="14" class="ml-1">mdi-chevron-down</v-icon>
                  </v-btn>
                </template>
                <v-list density="compact" class="siegu-list">
                  <v-list-item
                    v-for="mode in indexingModes"
                    :key="mode.value"
                    @click="$emit('set-indexing-mode', mode.value)"
                  >
                    <v-list-item-title
                      class="text-caption"
                      :class="{ 'font-weight-bold': performance.indexingMode === mode.value }"
                      >{{ $t('settings.mode_' + mode.value) }}</v-list-item-title
                    >
                  </v-list-item>
                </v-list>
              </v-menu>
            </template>
          </v-list-item>
        </div>
      </div>

      <v-divider class="my-6 border-subtle"></v-divider>

      <div>
        <div class="text-caption font-weight-bold text-zinc-muted mb-4 tracking-widest uppercase">
          {{ $t('settings.system_logs') }}
        </div>
        <v-sheet
          color="#f4f4f5"
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
            @click.stop="$emit('copy-logs')"
          >
            {{ $t('settings.copy_logs') }}
          </v-btn>
          <v-btn
            variant="text"
            size="small"
            class="text-none font-weight-bold"
            color="error"
            prepend-icon="mdi-trash-can-outline"
            @click.stop="$emit('clear-logs')"
          >
            {{ $t('settings.clear_logs') }}
          </v-btn>
        </div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import type { PerformanceConfig, LogEntry } from '@/types/settings'

defineProps<{
  performance: PerformanceConfig
  maxThreads: number
  isCleaning: boolean
  logs: LogEntry[]
}>()

defineEmits<{
  'cleanup-db': []
  'save-performance': []
  'set-indexing-mode': [mode: string]
  'clear-logs': []
  'copy-logs': []
}>()

const indexingModes = [{ value: 'immediate' }, { value: 'idle' }, { value: 'manual' }]

function getModeLabel(val: string): string {
  return val
}
</script>

<style scoped>
.debug-logs-sheet {
  max-height: 300px;
}
</style>
