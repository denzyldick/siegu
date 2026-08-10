<template>
  <div
    class="page-loading d-flex flex-column align-center justify-center py-16 text-center w-100"
    role="status"
    aria-live="polite"
  >
    <v-progress-circular indeterminate size="36" width="3" color="var(--color-text-secondary)" />
    <span
      v-if="label"
      class="mt-4 text-caption text-zinc-muted font-weight-medium tracking-widest text-uppercase"
    >
      {{ label }}
    </span>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  label?: string;
}>();
</script>
