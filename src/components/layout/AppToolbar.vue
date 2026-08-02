<script setup lang="ts">
import SearchBar from '@/components/search/SearchBar.vue'
import { useSearchStore } from '@/stores/search'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const searchStore = useSearchStore()

const sortOptions = [
  { value: 'newest', label: () => t('search.sort_newest'), icon: 'mdi-sort-calendar-descending' },
  { value: 'best', label: () => t('search.sort_best'), icon: 'mdi-star' },
  { value: 'random', label: () => t('search.sort_random'), icon: 'mdi-dice-multiple' },
] as const
</script>

<template>
  <v-app-bar elevation="0" color="surface" class="border-bottom-subtle px-2">
    <v-row class="px-2 align-center no-gutters">
      <v-col class="mx-2 flex-grow-1">
        <SearchBar />
      </v-col>
      <v-col cols="auto" class="ml-2">
        <v-tooltip v-if="sortOptions.length" location="top">
          <template #activator="{ props: tipProps }">
            <div v-bind="tipProps">
              <v-btn-toggle
                v-model="searchStore.sortOrder"
                density="compact"
                variant="outlined"
                color="primary"
                group
                mandatory
                class="sort-toggle"
                @update:model-value="searchStore.setSortOrder"
              >
                <v-btn
                  v-for="opt in sortOptions"
                  :key="opt.value"
                  :value="opt.value"
                  :aria-label="opt.label()"
                  size="small"
                >
                  <v-icon size="16">{{ opt.icon }}</v-icon>
                  <span class="ml-1 d-none d-sm-inline text-caption">{{ opt.label() }}</span>
                </v-btn>
              </v-btn-toggle>
            </div>
          </template>
          <span>{{ t('search.sort_label') }}</span>
        </v-tooltip>
      </v-col>
    </v-row>
  </v-app-bar>
</template>

<style scoped>
.sort-toggle {
  border-radius: 999px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.04);
}
.sort-toggle :deep(.v-btn) {
  border-radius: 0;
}
</style>
