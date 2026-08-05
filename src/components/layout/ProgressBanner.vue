<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { useScanStore } from '@/stores/scan';
import { normalizeIndexingCount, formatEta } from '@/composables/useMediaUtils';

const { t } = useI18n();
const scanStore = useScanStore();

const displayEta = computed(() => {
  if (scanStore.indexingEta && scanStore.indexingEta > 0) {
    return formatEta(scanStore.indexingEta);
  }
  return null;
});
</script>

<template>
  <v-slide-y-reverse-transition>
    <div
      v-if="scanStore.isActive || scanStore.stoppedMessage"
      class="progress-banner"
      data-tour="scan-progress"
    >
      <div class="progress-banner-inner px-4 py-2">
        <div class="d-flex align-center justify-space-between flex-wrap ga-2">
          <div class="d-flex align-center ga-2 min-width-0 flex-shrink-1" style="max-width: 55%">
            <v-progress-circular
              v-if="scanStore.isActive"
              indeterminate
              size="16"
              width="2"
              color="black"
            />
            <div class="text-caption font-weight-bold text-zinc-primary text-truncate">
              <template v-if="scanStore.stoppedMessage">
                <span>{{ t('sync.stopped_resume') }}</span>
              </template>
              <template v-else-if="scanStore.status === 'scanning'">
                <span>{{ t('sync.scanning') }}</span>
              </template>
              <template v-else-if="scanStore.status === 'indexing' || scanStore.indexingCount > 0">
                <span>{{ t('sync.indexing') }}: </span>
                <span class="text-zinc-muted font-weight-regular">
                  {{
                    t('sync.jobs_left', {
                      count: normalizeIndexingCount(scanStore.indexingCount).toLocaleString(),
                    })
                  }}
                </span>
              </template>
            </div>
          </div>
          <div class="d-flex align-center ga-3 flex-shrink-0">
            <span
              v-if="displayEta && !scanStore.stoppedMessage"
              class="text-caption text-zinc-muted"
            >
              ~{{ displayEta }}
            </span>
            <span
              v-else-if="scanStore.indexingCount > 0 && !scanStore.stoppedMessage"
              class="text-caption text-zinc-muted"
            >
              {{
                t('sync.jobs_left', {
                  count: normalizeIndexingCount(scanStore.indexingCount).toLocaleString(),
                })
              }}
            </span>
            <v-btn
              v-if="scanStore.isActive"
              size="x-small"
              variant="tonal"
              color="error"
              class="text-none"
              data-tour="stop-indexing"
              @click="scanStore.stop()"
            >
              {{ t('sync.stop') }}
            </v-btn>
          </div>
        </div>
      </div>
    </div>
  </v-slide-y-reverse-transition>
</template>

<style scoped>
.progress-banner {
  position: sticky;
  top: 0;
  z-index: 100;
  background: var(--color-bg-primary);
  border-bottom: 1px solid var(--color-border-subtle);
}

.progress-banner-inner {
  max-width: 1200px;
  margin: 0 auto;
}

.min-width-0 {
  min-width: 0;
}
</style>
