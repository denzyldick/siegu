<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSyncStore } from '@/stores/sync'

const { t } = useI18n()
const syncStore = useSyncStore()

const isVisible = computed(() => {
  return (
    syncStore.status !== 'idle' &&
    syncStore.status !== 'completed' &&
    syncStore.progress.total > 0
  )
})

const displayProgress = computed(() => {
  if (syncStore.progress.total === 0) return 0
  return Math.round((syncStore.progress.received / syncStore.progress.total) * 100)
})

function dismiss(): void {
  syncStore.status = 'idle'
}
</script>

<template>
  <v-fade-transition>
    <div v-if="isVisible" class="sync-banner-container">
      <v-sheet
        class="sync-banner d-flex align-center px-4 py-2 rounded-pill shadow-xl border-subtle"
        color="surface"
      >
        <v-progress-circular
          v-if="displayProgress === 0 || displayProgress === 100"
          indeterminate
          size="18"
          width="2"
          color="black"
          class="mr-3"
        />
        <div v-else class="mr-3 d-flex align-center">
          <v-progress-circular
            :model-value="displayProgress"
            size="22"
            width="3"
            color="black"
          >
            <span style="font-size: 8px; font-weight: bold">{{ displayProgress }}</span>
          </v-progress-circular>
        </div>
        <div
          class="text-caption font-weight-bold text-zinc-primary text-truncate pr-2"
          style="max-width: 220px"
        >
          {{ t('sync.status') }} {{ displayProgress }}%
        </div>
        <v-divider vertical class="mx-2 opacity-10" length="16" />
        <v-btn
          icon="mdi-close"
          variant="text"
          size="x-small"
          class="text-zinc-muted"
          @click="dismiss"
        />
      </v-sheet>
    </div>
  </v-fade-transition>
</template>

<style scoped>
.sync-banner-container {
  position: fixed;
  bottom: 100px;
  left: 0;
  right: 0;
  display: flex;
  justify-content: center;
  z-index: 3000;
  pointer-events: none;
}

.sync-banner {
  pointer-events: auto;
  min-width: 240px;
  max-width: 90vw;
  box-shadow: 0 10px 30px -5px rgba(0, 0, 0, 0.15) !important;
  border: 1px solid var(--color-border-subtle) !important;
}
</style>
