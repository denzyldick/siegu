<script setup lang="ts">
import { ref, onErrorCaptured } from 'vue';

const error = ref<Error | null>(null);

onErrorCaptured((err: unknown) => {
  error.value = err instanceof Error ? err : new Error(String(err));
  return false;
});

function retry(): void {
  error.value = null;
}
</script>

<template>
  <div
    v-if="error"
    class="error-boundary pa-8 d-flex flex-column align-center justify-center text-center"
  >
    <v-icon size="48" color="rgb(var(--v-theme-error))" class="mb-4"
      >mdi-alert-circle-outline</v-icon
    >
    <h3 class="text-h6 font-weight-bold text-high-emphasis mb-2">Something went wrong</h3>
    <p class="text-body-2 text-medium-emphasis mb-4 max-w-300">
      {{ error.message || 'An unexpected error occurred.' }}
    </p>
    <pre v-if="error.stack" class="error-stack text-caption text-disabled mb-6">{{
      error.stack
    }}</pre>
    <v-btn variant="flat" color="primary" class="" @click="retry"> Try Again </v-btn>
  </div>
  <slot v-else />
</template>

<style scoped>
.error-boundary {
  min-height: 200px;
}
.max-w-300 {
  max-width: 300px;
}
.error-stack {
  max-width: 560px;
  max-height: 160px;
  overflow: auto;
  text-align: left;
  white-space: pre-wrap;
  word-break: break-word;
  background: rgb(var(--v-theme-surface-light));
  border-radius: 8px;
  padding: 8px 10px;
  margin: 0 auto;
}
</style>
