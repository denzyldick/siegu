<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border-subtle overflow-hidden">
    <v-card-item class="bg-zinc-100 py-4">
      <template v-slot:prepend>
        <div class="siegu-icon-circle-dark mr-3">
          <v-icon color="#ffffff" size="small">mdi-speedometer</v-icon>
        </div>
      </template>
      <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
        $t('settings.performance')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-4">
      <div class="text-body-2 text-zinc-secondary mb-4">
        {{ $t('settings.performance_desc') }}
      </div>

      <v-row dense class="mb-2">
        <v-col v-for="preset in presets" :key="preset.value" cols="4" class="pa-1">
          <v-card
            variant="flat"
            class="preset-card rounded-lg"
            :class="{ 'preset-card-active': currentPreset === preset.value }"
            @click="$emit('apply-preset', preset.value)"
          >
            <v-card-text class="pa-2 text-center">
              <div class="text-body-2 font-weight-bold text-zinc-primary">
                {{ $t('settings.preset_' + preset.value) }}
              </div>
              <div class="text-caption text-zinc-muted preset-desc">
                {{ $t('settings.preset_' + preset.value + '_desc') }}
              </div>
            </v-card-text>
          </v-card>
        </v-col>
      </v-row>

      <div class="d-flex align-center justify-space-between mb-1">
        <div class="text-body-2 text-zinc-muted">
          {{
            currentPreset === 'custom'
              ? $t('settings.preset_custom_desc')
              : $t('settings.preset_' + currentPreset + '_desc')
          }}
        </div>
        <v-chip
          v-if="currentPreset === 'custom'"
          size="small"
          variant="tonal"
          color="primary"
          class="font-weight-bold"
        >
          {{ $t('settings.preset_custom') }}
        </v-chip>
      </div>

      <v-divider class="my-4 border-subtle"></v-divider>

      <div>
        <v-btn
          variant="text"
          size="small"
          class="text-none font-weight-bold pa-0"
          color="primary"
          @click="showAdvanced = !showAdvanced"
        >
          <v-icon size="16" class="mr-1">
            {{ showAdvanced ? 'mdi-chevron-up' : 'mdi-chevron-down' }}
          </v-icon>
          {{ $t('settings.' + (showAdvanced ? 'hide_advanced' : 'show_advanced')) }}
        </v-btn>

        <v-expand-transition>
          <div v-if="showAdvanced" class="pt-2">
            <div class="d-flex justify-space-between align-center mb-2">
              <div class="text-caption font-weight-bold text-zinc-primary">
                {{ $t('settings.batch_delay') }}
              </div>
              <v-chip
                size="small"
                color="#000000"
                variant="flat"
                class="font-weight-bold text-white"
              >
                {{ gapSeconds.toFixed(1) }}s
              </v-chip>
            </div>
            <v-slider
              :model-value="gapSeconds"
              :min="0"
              :max="2"
              :step="0.1"
              hide-details
              color="primary"
              track-color="#f4f4f5"
              @update:model-value="onGapChange"
            ></v-slider>
            <div class="text-caption text-zinc-muted mt-1">
              {{ $t('settings.batch_delay_desc') }}
            </div>

            <div class="d-flex justify-space-between align-center mb-2 mt-4">
              <div class="text-caption font-weight-bold text-zinc-primary">
                {{ $t('settings.memory_budget') }}
              </div>
              <v-chip
                size="small"
                color="#000000"
                variant="flat"
                class="font-weight-bold text-white"
              >
                {{
                  performance.memoryBudgetMb === 0
                    ? $t('settings.memory_budget_none')
                    : performance.memoryBudgetMb + ' MB'
                }}
              </v-chip>
            </div>
            <v-slider
              :model-value="memoryBudgetGigabytes"
              :min="0"
              :max="4"
              :step="0.25"
              hide-details
              color="primary"
              track-color="#f4f4f5"
              @update:model-value="onMemoryBudgetChange"
            ></v-slider>
            <div class="text-caption text-zinc-muted mt-1">
              {{ $t('settings.memory_budget_desc') }}
            </div>
          </div>
        </v-expand-transition>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import type { PerformanceConfig, PerformancePreset } from '@/types/settings';

const props = defineProps<{
  performance: PerformanceConfig;
  currentPreset: PerformancePreset;
}>();

const emit = defineEmits<{
  'apply-preset': [preset: string];
  'update-batch-delay': [valueMs: number];
  'update-memory-budget': [valueMb: number];
}>();

const presets = [{ value: 'low' }, { value: 'balanced' }, { value: 'full' }];

const showAdvanced = ref(false);

const gapSeconds = computed(() => props.performance.batchDelayMs / 1000);

const memoryBudgetGigabytes = computed(() => props.performance.memoryBudgetMb / 1024);

function onGapChange(value: number | [number, number]): void {
  const seconds = typeof value === 'number' ? value : value[0];
  emit('update-batch-delay', Math.round(seconds * 1000));
}

function onMemoryBudgetChange(value: number | [number, number]): void {
  const gb = typeof value === 'number' ? value : value[0];
  emit('update-memory-budget', Math.round(gb * 1024));
}
</script>

<style scoped>
.preset-card {
  border: 1px solid var(--color-border-subtle);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    background-color 0.18s ease;
}
.preset-card-active {
  border-color: var(--color-text-primary) !important;
  box-shadow: inset 0 2px 0 var(--color-text-primary);
}
.preset-desc {
  line-height: 1.25;
}
</style>
