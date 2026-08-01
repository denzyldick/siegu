<script setup lang="ts">
import { computed } from 'vue';
import { brandMeta } from '@/components/search/brands';

const props = defineProps<{
  name: string;
  size?: number;
}>();

const brand = computed(() => brandMeta(props.name));

const fill = computed(() => {
  const hex = brand.value?.hex ?? '#000000';
  if (hex === 'FFFFFF') return 'currentColor';
  return hex;
});
</script>

<template>
  <svg
    v-if="brand"
    :width="size ?? 14"
    :height="size ?? 14"
    viewBox="0 0 24 24"
    class="brand-icon"
    role="img"
    aria-label="brand"
  >
    <path :d="brand.path" :fill="fill" />
  </svg>
  <v-icon v-else :size="size ?? 14">mdi-camera</v-icon>
</template>

<style scoped>
.brand-icon {
  display: inline-block;
  flex-shrink: 0;
}
</style>
