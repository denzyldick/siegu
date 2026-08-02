<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSearchStore } from '@/stores/search'
import { useUiStore } from '@/stores/ui'

const { t } = useI18n()
const searchStore = useSearchStore()
const uiStore = useUiStore()

const people = computed(() => searchStore.facets?.people ?? [])

function selectPerson(person: { id: string; name: string | null }): void {
  searchStore.addFilter({ type: 'person', value: String(person.id), label: person.name || '?' })
  searchStore.clearQuery()
  uiStore.setPage('home')
}
</script>

<template>
  <div v-if="people.length > 0" class="people-rail px-4 pt-3">
    <div class="people-rail-inner" style="max-width: 980px; margin: 0 auto">
      <div class="d-flex align-center mb-2">
        <span class="text-overline text-zinc-muted tracking-widest">
          {{ t('people_rail.title') }}
        </span>
        <v-spacer></v-spacer>
        <span class="text-caption text-zinc-muted">{{ people.length }}</span>
      </div>
      <div class="people-scroll d-flex ga-3 overflow-x-auto pb-2">
        <button
          v-for="person in people"
          :key="person.id"
          class="person-chip d-flex flex-column align-center cursor-pointer"
          @click="selectPerson(person)"
        >
          <v-avatar size="56" class="border-subtle mb-1">
            <v-img :src="person.encoded || person.representative_crop || ''" cover></v-img>
          </v-avatar>
          <span class="text-caption text-zinc-primary text-truncate w-100 text-center font-weight-bold person-name">
            {{ person.name || t('people_rail.unnamed') }}
          </span>
          <span class="text-caption text-zinc-muted person-count">
            {{ person.count }}
          </span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.people-rail {
  border-bottom: 1px solid rgba(0, 0, 0, 0.05);
}

.people-scroll {
  scrollbar-width: thin;
}

.person-chip {
  background: transparent;
  border: none;
  padding: 4px 6px;
  border-radius: 12px;
  min-width: 72px;
  transition: background 0.2s ease, transform 0.2s ease;
}

.person-chip:hover {
  background: rgba(0, 0, 0, 0.04);
}

.person-chip:active {
  transform: scale(0.95);
}

.person-name {
  max-width: 72px;
}

.person-count {
  font-size: 10px;
}
</style>
