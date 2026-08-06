<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { useSearchStore } from '@/stores/search';

const { t } = useI18n();
const searchStore = useSearchStore();
</script>

<template>
  <div class="person-match-controls d-flex align-center flex-wrap ga-1">
    <div v-if="searchStore.personCount > 1" class="person-toggle-group" role="group">
      <button
        class="person-toggle"
        :class="{ active: searchStore.personMatch === 'and' }"
        :aria-pressed="searchStore.personMatch === 'and'"
        @click="searchStore.setPersonMatch('and')"
      >
        <v-icon size="13" class="mr-1">mdi-account-group</v-icon>
        {{ t('search.person_match_and') }}
      </button>
      <button
        class="person-toggle"
        :class="{ active: searchStore.personMatch === 'or' }"
        :aria-pressed="searchStore.personMatch === 'or'"
        @click="searchStore.setPersonMatch('or')"
      >
        <v-icon size="13" class="mr-1">mdi-account-multiple-outline</v-icon>
        {{ t('search.person_match_or') }}
      </button>
    </div>
    <button
      class="person-toggle person-toggle--solo"
      :class="{ active: searchStore.personAlone }"
      :aria-pressed="searchStore.personAlone"
      @click="searchStore.togglePersonAlone()"
    >
      <v-icon size="13" class="mr-1">mdi-account-off-outline</v-icon>
      {{ t('search.person_alone') }}
    </button>
  </div>
</template>

<style scoped>
.person-toggle-group {
  display: inline-flex;
  border-radius: var(--radius-pill);
  overflow: hidden;
  border: 1px solid var(--color-border-subtle);
}

.person-toggle {
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary);
  background: var(--color-bg-hover);
  border: none;
  padding: 4px 12px;
  cursor: pointer;
  user-select: none;
  transition: all 0.15s ease;
}

.person-toggle + .person-toggle {
  border-left: 1px solid var(--color-border-subtle);
}

.person-toggle:hover {
  color: var(--color-text-primary);
}

.person-toggle.active {
  color: var(--color-text-primary);
  background: color-mix(in srgb, var(--color-text-primary) 12%, transparent);
}

.person-toggle--solo {
  border-radius: var(--radius-pill);
  border: 1px solid var(--color-border-subtle);
}

.person-toggle--solo.active {
  border-color: var(--color-text-primary);
}
</style>
