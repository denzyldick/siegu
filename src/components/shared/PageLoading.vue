<template>
  <div
    class="page-loading d-flex flex-column align-center justify-center py-16 text-center w-100"
    role="status"
    aria-live="polite"
  >
    <v-progress-circular
      indeterminate
      size="36"
      width="3"
      color="rgba(var(--v-theme-on-surface), 0.7)"
    />
    <span
      v-if="label"
      class="mt-4 text-caption text-disabled font-weight-medium tracking-widest text-uppercase"
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
