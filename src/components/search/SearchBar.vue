<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSearchStore } from '@/stores/search'
import { getFaceImageSrc, formatMonthLabel } from '@/composables/useMediaUtils'
import type { FacetGroup } from '@/types/search'

const emit = defineEmits<{
  (e: 'search', query: string): void
  (e: 'advanced'): void
}>()

const { t } = useI18n()
const searchStore = useSearchStore()

const searchWrapRef = ref<HTMLElement | null>(null)
const dropdownOpen = ref(false)

const facets = computed(() => searchStore.facets)
const stats = computed(() => facets.value?.stats ?? null)

const q = computed(() => searchStore.query.trim().toLowerCase())

function matches(value: string): boolean {
  return value.toLowerCase().includes(q.value)
}

const namedPeople = computed(() =>
  (facets.value?.people ?? []).filter((p) => !q.value || matches(p.name ?? '')),
)

const peopleRow = computed(() =>
  q.value
    ? namedPeople.value
    : namedPeople.value.length
      ? namedPeople.value.slice(0, 12)
      : (facets.value?.unnamed_faces ?? []).slice(0, 12),
)

const otherFaces = computed(() =>
  !q.value && namedPeople.value.length ? (facets.value?.unnamed_faces ?? []).slice(0, 8) : [],
)

const locations = computed(() =>
  (facets.value?.locations ?? []).filter((l) => !q.value || matches(l.name)),
)

const tags = computed(() => (facets.value?.tags ?? []).filter((l) => !q.value || matches(l.name)))

const months = computed(() =>
  (facets.value?.months ?? []).filter((m) => !q.value || matches(formatMonthLabel(m.name))),
)

const recentSearches = computed(() => searchStore.recentSearches)

function faceSrc(person: FacetGroup): string {
  return getFaceImageSrc(person.representative_crop, person.encoded)
}

function openDropdown(): void {
  dropdownOpen.value = true
  if (!searchStore.facets) {
    searchStore.loadFacets()
  }
}

function closeDropdown(): void {
  dropdownOpen.value = false
}

function selectPerson(person: FacetGroup): void {
  searchStore.addFilter({
    type: 'person',
    value: person.id,
    label: person.name ?? '',
  })
  closeDropdown()
}

function selectLocation(name: string): void {
  searchStore.addFilter({ type: 'location', value: name, label: name })
  closeDropdown()
}

function selectTag(name: string): void {
  searchStore.addFilter({ type: 'tag', value: name, label: name })
  closeDropdown()
}

function selectMonth(ym: string): void {
  searchStore.addFilter({ type: 'month', value: ym, label: formatMonthLabel(ym) })
  closeDropdown()
}

function runSearch(): void {
  const term = searchStore.query.trim()
  if (!term) return
  searchStore.addRecentSearch(term)
  emit('search', term)
  closeDropdown()
}

function selectRecent(term: string): void {
  searchStore.setQuery(term)
  emit('search', term)
  closeDropdown()
}

function clearAll(): void {
  searchStore.clearQuery()
  searchStore.clearFilters()
  closeDropdown()
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter') {
    runSearch()
  } else if (event.key === 'Escape') {
    closeDropdown()
  }
}

function onDocumentClick(event: MouseEvent): void {
  if (!dropdownOpen.value) return
  const target = event.target as Node
  if (searchWrapRef.value && !searchWrapRef.value.contains(target)) {
    closeDropdown()
  }
}

onMounted(() => {
  searchStore.loadFacets()
  document.addEventListener('click', onDocumentClick)
})

onUnmounted(() => {
  document.removeEventListener('click', onDocumentClick)
})
</script>

<template>
  <div ref="searchWrapRef" class="search-wrapper">
    <div class="search-field" data-tour="search" @click="openDropdown">
      <v-icon size="20" class="search-icon" color="#a1a1aa">mdi-magnify</v-icon>
      <input
        v-model="searchStore.query"
        class="search-input"
        :placeholder="t('search.placeholder')"
        @focus="openDropdown"
        @keydown="onKeydown"
      />
      <v-icon
        v-if="searchStore.query || searchStore.hasFilters"
        size="18"
        color="#a1a1aa"
        class="cursor-pointer"
        @click.stop="clearAll"
      >
        mdi-close-circle
      </v-icon>
      <v-tooltip location="bottom" max-width="280">
        <template v-slot:activator="{ props }">
          <v-icon v-bind="props" size="18" color="#a1a1aa" class="cursor-pointer">
            mdi-help-circle-outline
          </v-icon>
        </template>
        <div class="pa-2">
          <div class="text-caption font-weight-bold mb-2">{{ t('search.help_title') }}</div>
          <div class="text-caption mb-1">{{ t('search.help_desc') }}</div>
          <div class="text-caption mb-1">&#8226; {{ t('search.help_tags') }}</div>
          <div class="text-caption mb-1">&#8226; {{ t('search.help_people') }}</div>
          <div class="text-caption mb-1">&#8226; {{ t('search.help_location') }}</div>
          <div class="text-caption mb-1">&#8226; {{ t('search.help_date') }}</div>
          <div class="text-caption mb-1">&#8226; {{ t('search.help_caption') }}</div>
          <div class="text-caption">&#8226; {{ t('search.help_ocr') }}</div>
        </div>
      </v-tooltip>
    </div>

    <div v-if="dropdownOpen" class="search-dropdown">
      <v-progress-circular
        v-if="!facets && !searchStore.facetsLoading"
        indeterminate
        size="20"
        width="2"
        class="d-block mx-auto my-4"
      />

      <template v-else>
        <div v-if="q" class="pa-1">
          <div class="run-search-item" @click="runSearch">
            <v-icon size="18" class="mr-2" color="#0ea5e9">mdi-text-search</v-icon>
            <span>{{ t('search.enter_to_search', { query: searchStore.query.trim() }) }}</span>
          </div>
        </div>

        <template v-if="!q && recentSearches.length">
          <div class="text-overline pa-2 pb-1 text-zinc-muted">{{ t('search.recent') }}</div>
          <div class="pa-1">
            <div
              v-for="term in recentSearches"
              :key="term"
              class="dropdown-item"
              @click="selectRecent(term)"
            >
              <v-icon size="18" class="mr-2" color="#a1a1aa">mdi-history</v-icon>
              <span class="flex-1 ellipsis">{{ term }}</span>
              <v-icon size="14" color="#a1a1aa" class="mr-1">mdi-arrow-up-left</v-icon>
            </div>
          </div>
          <v-divider class="border-subtle" />
        </template>

        <template v-if="peopleRow.length">
          <div class="text-overline pa-2 pb-1 text-zinc-muted">{{ t('search.people') }}</div>
          <div class="face-row pa-1">
            <div
              v-for="person in peopleRow"
              :key="person.id"
              class="face-card"
              @click="selectPerson(person)"
            >
              <v-avatar size="52" rounded="lg" class="face-avatar">
                <v-img v-if="faceSrc(person)" :src="faceSrc(person)" cover />
                <v-icon v-else>mdi-account</v-icon>
              </v-avatar>
              <div class="face-name ellipsis">{{ person.name ?? t('people.unnamed') }}</div>
              <div class="face-count">{{ person.count }}</div>
            </div>
          </div>
        </template>

        <template v-if="otherFaces.length">
          <div class="text-overline pa-2 pb-1 text-zinc-muted">{{ t('search.unnamed_faces') }}</div>
          <div class="pa-1">
            <div
              v-for="person in otherFaces"
              :key="person.id"
              class="dropdown-item"
              @click="selectPerson(person)"
            >
              <v-avatar size="28" rounded="lg" class="mr-2">
                <v-img v-if="faceSrc(person)" :src="faceSrc(person)" cover />
                <v-icon v-else size="16">mdi-account</v-icon>
              </v-avatar>
              <span class="flex-1">{{ t('people.unnamed') }}</span>
              <span class="text-zinc-muted">({{ person.count }})</span>
            </div>
          </div>
        </template>

        <template v-if="locations.length">
          <div class="text-overline pa-2 pb-1 text-zinc-muted">{{ t('search.locations') }}</div>
          <div class="pa-1">
            <div
              v-for="location in locations"
              :key="location.name"
              class="dropdown-item"
              @click="selectLocation(location.name)"
            >
              <v-icon size="18" class="mr-2" color="#f59e0b">mdi-map-marker</v-icon>
              <span class="flex-1 ellipsis">{{ location.name }}</span>
              <span class="text-zinc-muted">({{ location.count }})</span>
            </div>
          </div>
        </template>

        <template v-if="tags.length">
          <div class="text-overline pa-2 pb-1 text-zinc-muted">{{ t('search.tags') }}</div>
          <div class="pa-1">
            <div v-for="tag in tags" :key="tag.name" class="dropdown-item" @click="selectTag(tag.name)">
              <v-icon size="18" class="mr-2" color="#10b981">mdi-tag</v-icon>
              <span class="flex-1 ellipsis">{{ tag.name }}</span>
              <span class="text-zinc-muted">({{ tag.count }})</span>
            </div>
          </div>
        </template>

        <template v-if="months.length">
          <div class="text-overline pa-2 pb-1 text-zinc-muted">{{ t('search.dates') }}</div>
          <div class="pa-1">
            <div v-for="month in months" :key="month.name" class="dropdown-item" @click="selectMonth(month.name)">
              <v-icon size="18" class="mr-2" color="#8b5cf6">mdi-calendar-month</v-icon>
              <span class="flex-1 ellipsis">{{ formatMonthLabel(month.name) }}</span>
              <span class="text-zinc-muted">({{ month.count }})</span>
            </div>
          </div>
        </template>

        <div v-if="!q && !peopleRow.length && !locations.length && !tags.length && !months.length" class="pa-3 text-center">
          <div class="text-body-2 text-zinc-muted">{{ t('search.no_data') }}</div>
        </div>

        <div v-if="q && !peopleRow.length && !otherFaces.length && !locations.length && !tags.length && !months.length" class="pa-3 text-center">
          <div class="text-body-2 text-zinc-muted">{{ t('search.no_matches', { query: searchStore.query.trim() }) }}</div>
        </div>

        <div v-if="stats" class="search-stats pa-2">
          <span>{{ t('search.photos_count', { count: stats.photos.toLocaleString() }) }}</span>
          <span>·</span>
          <span>{{ t('search.videos_count', { count: stats.videos.toLocaleString() }) }}</span>
          <span>·</span>
          <span>{{ t('search.faces_count', { count: stats.faces.toLocaleString() }) }}</span>
        </div>

        <v-divider class="border-subtle" />
        <div class="pa-2">
          <div class="advanced-btn" @click="emit('advanced')">
            <v-icon size="18" class="mr-2">mdi-tune-variant</v-icon>
            <span>{{ t('search.expand') }}</span>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.search-wrapper {
  position: relative;
  width: 100%;
}

.search-field {
  display: flex;
  align-items: center;
  gap: 8px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(128, 128, 128, 0.25);
  border-radius: 12px;
  padding: 0 12px;
  height: 40px;
  cursor: text;
}

.search-field:focus-within {
  border-color: rgb(var(--v-theme-primary));
}

.search-icon {
  flex-shrink: 0;
}

.search-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: rgb(var(--v-theme-on-surface));
  font-size: 14px;
}

.search-input::placeholder {
  color: rgba(128, 128, 128, 0.8);
}

.search-dropdown {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  right: 0;
  z-index: 2000;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(128, 128, 128, 0.25);
  border-radius: 16px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.35);
  overflow-y: auto;
  max-height: min(70vh, 560px);
  padding: 6px 0;
}

.dropdown-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  border-radius: 10px;
  cursor: pointer;
  user-select: none;
}

.dropdown-item:hover {
  background: rgba(128, 128, 128, 0.1);
}

.run-search-item {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  border-radius: 10px;
  cursor: pointer;
  user-select: none;
  font-weight: 600;
}

.run-search-item:hover {
  background: rgba(128, 128, 128, 0.1);
}

.face-row {
  display: flex;
  gap: 10px;
  overflow-x: auto;
  padding: 4px 8px;
}

.face-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  min-width: 68px;
  padding: 6px 4px;
  border-radius: 12px;
  cursor: pointer;
  user-select: none;
}

.face-card:hover {
  background: rgba(128, 128, 128, 0.1);
}

.face-avatar {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
}

.face-name {
  font-size: 12px;
  max-width: 68px;
}

.face-count {
  font-size: 11px;
  color: rgba(128, 128, 128, 0.9);
}

.ellipsis {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.search-stats {
  display: flex;
  justify-content: center;
  gap: 8px;
  font-size: 12px;
  color: rgba(128, 128, 128, 0.9);
}

.advanced-btn {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  border-radius: 10px;
  cursor: pointer;
  user-select: none;
  font-weight: 600;
  color: rgb(var(--v-theme-primary));
}

.advanced-btn:hover {
  background: rgba(128, 128, 128, 0.1);
}
</style>
