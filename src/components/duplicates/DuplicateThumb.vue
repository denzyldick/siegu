<script setup lang="ts">
import { computed } from 'vue';
import { useMediaUrl } from '@/composables/useMediaUrl';
import type { MediaItem } from '@/types/media';

const props = defineProps<{
  id: string | number;
  location: string;
}>();

const item = computed<MediaItem | null>(() => ({
  id: props.id,
  location: props.location,
} as unknown as MediaItem));

const { mediaSrcRef } = useMediaUrl();
const src = mediaSrcRef(item, 'thumb');
</script>

<template>
  <img v-if="src" :src="src" class="duplicate-thumb" alt="" />
  <div v-else class="duplicate-thumb"></div>
</template>

<style scoped>
.duplicate-thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  background: rgba(var(--v-theme-on-surface), 0.05);
}
</style>