<script setup lang="ts">
import { ref, onErrorCaptured } from 'vue'

const error = ref<Error | null>(null)

onErrorCaptured((err: unknown) => {
  error.value = err instanceof Error ? err : new Error(String(err))
  return false
})

function retry(): void {
  error.value = null
}
</script>

<template>
  <div v-if="error" class="error-boundary pa-8 d-flex flex-column align-center justify-center text-center">
    <v-icon size="48" color="var(--color-error)" class="mb-4">mdi-alert-circle-outline</v-icon>
    <h3 class="text-h6 font-weight-bold text-zinc-primary mb-2">Something went wrong</h3>
    <p class="text-body-2 text-zinc-secondary mb-6 max-w-300">
      {{ error.message || 'An unexpected error occurred.' }}
    </p>
    <v-btn variant="flat" color="black" class="siegu-btn" @click="retry">
      Try Again
    </v-btn>
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
</style>
