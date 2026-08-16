<script setup lang="ts">
import { useSyncStore } from '@/stores/sync';

defineProps<{
  progress: number;
  itemsCompleted: number;
  itemsTotal: number;
}>();

const syncStore = useSyncStore();
</script>

<template>
  <div class="d-flex flex-column align-center w-100 ga-2">
    <div v-if="syncStore.currentFile" class="d-flex flex-column align-center w-100 ga-2">
      <img
        v-if="syncStore.currentFile.thumbnail"
        :src="syncStore.currentFile.thumbnail"
        class="rounded"
        alt=""
        style="width: 96px; height: 96px; object-fit: cover"
      />
      <span class="text-caption text-medium-emphasis text-truncate w-100 text-center">
        {{ syncStore.currentFile.filename }}
      </span>
      <div class="d-flex align-center ga-2 w-100">
        <v-progress-linear
          :model-value="progress"
          color="success"
          height="6"
          rounded
          class="flex-grow-1"
        />
        <span
          v-if="itemsTotal > 0"
          class="text-caption font-weight-bold"
          style="color: rgb(var(--v-theme-success))"
        >
          {{ itemsCompleted }}/{{ itemsTotal }}
        </span>
      </div>
    </div>
    <v-progress-linear v-else indeterminate color="success" height="4" class="w-100" />
  </div>
</template>
