<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSearchStore } from '@/stores/search'
import { getFaceImageSrc } from '@/composables/useMediaUtils'
import DateRangePicker from '@/components/search/DateRangePicker.vue'
import type { FacetType } from '@/types/search'

const { t } = useI18n()
const searchStore = useSearchStore()

const props = defineProps<{
  modelValue: boolean;
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
}>()

const isOpen = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit('update:modelValue', value),
})

const facets = computed(() => searchStore.facets)
const active = computed(() => searchStore.activeFilters)

function isActive(type: FacetType, value: string): boolean {
  return active.value.some((f) => f.type === type && f.value === value)
}

function toggle(type: FacetType, value: string, label: string): void {
  if (isActive(type, value)) {
    searchStore.removeFilter(type)
  } else {
    searchStore.addFilter({ type, value, label })
  }
}

function formatDateLabel(range: [string, string]): string {
  const locale = localStorage.getItem('siegu_language') || 'en'
  const fmt = (d: string) =>
    new Date(`${d}T00:00:00`).toLocaleDateString(locale, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    })
  return range[0] === range[1] ? fmt(range[0]) : `${fmt(range[0])} — ${fmt(range[1])}`
}

function onDateRangeChange(range: [string, string] | null): void {
  searchStore.setDateRange(range)
  if (range) {
    searchStore.addFilter({ type: 'date', value: `${range[0]}|${range[1]}`, label: formatDateLabel(range) })
  } else {
    searchStore.removeFilter('date')
  }
}

const activeCount = computed(() => active.value.length)
</script>

<template>
  <v-dialog v-model="isOpen" :fullscreen="$vuetify.display.mobile" max-width="720" content-class="rounded-xl">
    <v-card class="bg-siegu-card" rounded="xl">
      <v-card-title class="d-flex align-center justify-space-between py-3">
        <span class="text-h6">{{ t('search.advanced.title') }}</span>
        <div class="d-flex align-center ga-2">
          <v-chip v-if="activeCount" size="small" color="primary" variant="flat">
            {{ activeCount }}
          </v-chip>
          <v-btn
            v-if="searchStore.hasFilters"
            size="small"
            variant="tonal"
            color="error"
            @click="searchStore.clearFilters()"
          >
            {{ t('search.advanced.clear_all') }}
          </v-btn>
          <v-btn size="small" icon variant="text" @click="isOpen = false">
            <v-icon>mdi-close</v-icon>
          </v-btn>
        </div>
      </v-card-title>

      <v-card-text class="pt-0 filter-panel-scroll" style="max-height: 70vh; overflow-y: auto">
        <div class="mb-4">
          <div class="text-subtitle-2 mb-2 text-siegu-subtle">{{ t('search.advanced.media_type') }}</div>
          <div class="d-flex ga-2 flex-wrap">
            <v-chip
              :color="searchStore.mediaFilters.favoritesOnly ? 'primary' : ''"
              variant="tonal"
              @click="searchStore.toggleFavoriteOnly()"
            >
              <v-icon start>mdi-star</v-icon>
              {{ t('search.advanced.favorites') }}
              <span v-if="facets?.stats" class="ml-1 opacity-75">({{ facets.stats.favorites }})</span>
            </v-chip>
            <v-chip
              :color="searchStore.mediaFilters.videosOnly ? 'primary' : ''"
              variant="tonal"
              @click="searchStore.toggleVideoOnly()"
            >
              <v-icon start>mdi-video</v-icon>
              {{ t('search.advanced.videos') }}
              <span v-if="facets?.stats" class="ml-1 opacity-75">({{ facets.stats.videos }})</span>
            </v-chip>
          </div>
        </div>

        <div v-if="facets?.people?.length" class="mb-4">
          <div class="text-subtitle-2 mb-2 text-siegu-subtle">{{ t('search.people') }}</div>
          <div class="filter-list">
            <div
              v-for="person in facets.people"
              :key="person.id"
              class="filter-item"
              :class="{ 'filter-item--active': isActive('person', person.id) }"
              @click="toggle('person', person.id, person.name ?? '')"
            >
              <v-checkbox
                :model-value="isActive('person', person.id)"
                density="compact"
                hide-details
                class="mr-1"
              />
              <v-avatar size="32" class="mr-2">
                <v-img
                  v-if="getFaceImageSrc(person.representative_crop, person.encoded)"
                  :src="getFaceImageSrc(person.representative_crop, person.encoded)"
                />
                <v-icon v-else>mdi-account</v-icon>
              </v-avatar>
              <span class="flex-1">{{ person.name ?? t('people.unnamed') }}</span>
              <span class="text-siegu-subtle">({{ person.count }})</span>
            </div>
          </div>
        </div>

        <div v-if="facets?.unnamed_faces?.length" class="mb-4">
          <div class="text-subtitle-2 mb-2 text-siegu-subtle">{{ t('search.unnamed_faces') }}</div>
          <div class="filter-list">
            <div
              v-for="person in facets.unnamed_faces"
              :key="person.id"
              class="filter-item"
              :class="{ 'filter-item--active': isActive('person', person.id) }"
              @click="toggle('person', person.id, '')"
            >
              <v-checkbox
                :model-value="isActive('person', person.id)"
                density="compact"
                hide-details
                class="mr-1"
              />
              <v-avatar size="32" class="mr-2">
                <v-img
                  v-if="getFaceImageSrc(person.representative_crop, person.encoded)"
                  :src="getFaceImageSrc(person.representative_crop, person.encoded)"
                />
                <v-icon v-else>mdi-account</v-icon>
              </v-avatar>
              <span class="flex-1">{{ t('people.unnamed') }}</span>
              <span class="text-siegu-subtle">({{ person.count }})</span>
            </div>
          </div>
        </div>

        <div v-if="facets?.locations?.length" class="mb-4">
          <div class="text-subtitle-2 mb-2 text-siegu-subtle">{{ t('search.locations') }}</div>
          <div class="filter-list">
            <div
              v-for="location in facets.locations"
              :key="location.name"
              class="filter-item"
              :class="{ 'filter-item--active': isActive('location', location.name) }"
              @click="toggle('location', location.name, location.name)"
            >
              <v-checkbox
                :model-value="isActive('location', location.name)"
                density="compact"
                hide-details
                class="mr-1"
              />
              <v-icon size="18" class="mr-2">mdi-map-marker</v-icon>
              <span class="flex-1">{{ location.name }}</span>
              <span class="text-siegu-subtle">({{ location.count }})</span>
            </div>
          </div>
        </div>

        <div v-if="facets?.tags?.length" class="mb-4">
          <div class="text-subtitle-2 mb-2 text-siegu-subtle">{{ t('search.tags') }}</div>
          <div class="filter-list">
            <div
              v-for="tag in facets.tags"
              :key="tag.name"
              class="filter-item"
              :class="{ 'filter-item--active': isActive('tag', tag.name) }"
              @click="toggle('tag', tag.name, tag.name)"
            >
              <v-checkbox
                :model-value="isActive('tag', tag.name)"
                density="compact"
                hide-details
                class="mr-1"
              />
              <v-icon size="18" class="mr-2">mdi-tag</v-icon>
              <span class="flex-1">{{ tag.name }}</span>
              <span class="text-siegu-subtle">({{ tag.count }})</span>
            </div>
          </div>
        </div>

        <div v-if="facets?.stats && facets.stats.photos">
          <div class="text-subtitle-2 mb-2 text-siegu-subtle">{{ t('search.dates') }}</div>
          <DateRangePicker
            :model-value="searchStore.dateRange"
            @update:model-value="onDateRangeChange"
          />
        </div>
      </v-card-text>

      <v-card-actions class="px-4 pb-4">
        <v-spacer />
        <v-btn variant="flat" color="primary" @click="isOpen = false">
          {{ t('common.done') }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<style scoped>
.filter-list {
  display: flex;
  flex-direction: column;
}

.filter-item {
  display: flex;
  align-items: center;
  padding: 4px 8px;
  border-radius: 8px;
  cursor: pointer;
  user-select: none;
}

.filter-item:hover {
  background: rgba(255, 255, 255, 0.06);
}

.filter-item--active {
  background: rgba(99, 102, 241, 0.14);
}
</style>
