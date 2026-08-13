<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { Person } from '@/types/person';
import { normalizeIndexingCount } from '@/composables/useMediaUtils';
import UnnamedFacesGrid from './UnnamedFacesGrid.vue';

const props = defineProps<{
  faces: Person[];
  indexingCount: number;
}>();

const emit = defineEmits<{
  startIndexing: [];
  viewCluster: [group: Person];
  promptName: [group: Person];
}>();

const { t } = useI18n();

const formattedCount = computed(() =>
  normalizeIndexingCount(props.indexingCount).toLocaleString(
    localStorage.getItem('siegu_language') || 'en',
  ),
);
</script>

<template>
  <div class="people-manage-panel animate-fade-in px-2">
    <div class="d-flex align-center ga-3 mb-4 flex-wrap">
      <v-btn
        v-if="indexingCount === 0"
        variant="flat"
        color="primary"
        class=""
        prepend-icon="mdi-face-recognition"
        @click="emit('startIndexing')"
      >
        {{ t('people.index_faces') }}
      </v-btn>
      <div v-else class="d-flex align-center ga-2">
        <v-progress-circular
          indeterminate
          size="16"
          width="2"
          color="rgb(var(--v-theme-on-surface))"
        ></v-progress-circular>
        <span class="text-caption text-disabled">
          {{ t('people.indexing_remaining', { count: formattedCount }) }}
        </span>
      </div>
    </div>

    <UnnamedFacesGrid
      :faces="faces"
      @view-cluster="(group) => emit('viewCluster', group)"
      @prompt-name="(group) => emit('promptName', group)"
    />
  </div>
</template>
