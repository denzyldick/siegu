<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { useSearchStore } from '@/stores/search';

const { t } = useI18n();
const searchStore = useSearchStore();
</script>

<template>
  <div class="person-match-controls d-flex align-center flex-wrap ga-1">
    <v-btn-toggle
      v-if="searchStore.personCount > 1"
      :model-value="searchStore.personMatch"
      @update:model-value="
        (v: string | undefined) => v && searchStore.setPersonMatch(v as 'and' | 'or')
      "
      variant="outlined"
      density="compact"
      divided
    >
      <v-btn size="small" value="and">
        <v-icon size="13" class="mr-1">mdi-account-group</v-icon>
        {{ t('search.person_match_and') }}
      </v-btn>
      <v-btn size="small" value="or">
        <v-icon size="13" class="mr-1">mdi-account-multiple-outline</v-icon>
        {{ t('search.person_match_or') }}
      </v-btn>
    </v-btn-toggle>
    <v-btn
      size="small"
      variant="outlined"
      class="person-toggle--solo"
      :class="{ active: searchStore.personAlone }"
      @click="searchStore.togglePersonAlone()"
    >
      <v-icon size="13" class="mr-1">mdi-account-off-outline</v-icon>
      {{ t('search.person_alone') }}
    </v-btn>
  </div>
</template>

<style scoped>
.person-toggle--solo.active {
  border-color: rgb(var(--v-theme-on-surface));
}
</style>
