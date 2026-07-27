<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useScanStore } from '@/stores/scan'
import { useSearchStore } from '@/stores/search'
import { useAppStore } from '@/stores/app'
import { normalizeIndexingCount } from '@/composables/useMediaUtils'

interface Props {
  isActive: boolean
  statusLabel: string
}

defineProps<Props>()
const emit = defineEmits<{
  (e: 'scan'): void
  (e: 'search', query: string): void
}>()

const { t } = useI18n()
const scanStore = useScanStore()
const searchStore = useSearchStore()
const appStore = useAppStore()

const isMobile = computed(() => appStore.os === 'android' || appStore.os === 'ios')
const hasActiveFilters = computed(() => false)
const selectedSearch = ref<string | null>(null)

function iconForType(type: string): string {
  if (type === 'person') return 'mdi-account'
  if (type === 'location') return 'mdi-map-marker'
  if (type === 'date') return 'mdi-calendar-month'
  return 'mdi-tag'
}

function iconColor(type: string): string {
  if (type === 'person') return '#0ea5e9'
  if (type === 'location') return '#f59e0b'
  if (type === 'date') return '#8b5cf6'
  return '#10b981'
}

function formatIndexingCount(count: number): string {
  return normalizeIndexingCount(count).toLocaleString()
}

function handleSearchKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter') {
    searchStore.addRecentSearch(searchStore.query)
    emit('search', searchStore.query)
  }
}

function handleSearchSelect(value: unknown): void {
  if (value && typeof value === 'object' && value !== null && 'title' in value) {
    const title = (value as { title: string }).title
    searchStore.setQuery(title)
    searchStore.addRecentSearch(title)
    emit('search', title)
  }
}
</script>

<template>
  <v-app-bar
    elevation="0"
    color="surface"
    class="border-bottom-subtle px-2"
  >
    <v-row class="px-2 align-center no-gutters">
      <v-col cols="auto">
        <v-menu offset-y transition="scale-transition">
          <template v-slot:activator="{ props }">
            <v-btn
              v-bind="props"
              color="#000000"
              theme="dark"
              variant="flat"
              :class="isMobile ? 'px-2' : 'px-4'"
              height="40"
              rounded="lg"
              data-tour="scan-button"
            >
              <div class="d-flex align-center">
                <div :class="isMobile ? '' : 'mr-2'">
                  <v-progress-circular
                    v-if="isActive"
                    indeterminate
                    size="16"
                    width="2"
                    color="white"
                  />
                  <v-icon v-else size="18" color="white">mdi-sync</v-icon>
                </div>
                <span v-if="!isMobile" class="text-white font-weight-bold">{{ statusLabel }}</span>
              </div>
            </v-btn>
          </template>
          <v-card
            min-width="320"
            border
            class="mt-2 border-subtle overflow-hidden"
            color="surface"
            rounded="xl"
          >
            <div class="bg-zinc-50 pa-4 border-bottom-subtle">
              <div class="text-overline font-weight-black text-zinc-muted mb-1">
                {{ t('sync.status_label') }}
              </div>
              <div class="d-flex align-center justify-space-between">
                <div class="text-subtitle-1 font-weight-bold text-zinc-primary">
                  {{ t('app.name') }} {{ t('app.sync') }}
                </div>
                <v-chip
                  v-if="isActive"
                  size="x-small"
                  color="black"
                  variant="flat"
                  class="text-white"
                >
                  {{ statusLabel }}
                </v-chip>
              </div>
            </div>
            <v-card-text class="pa-4">
              <v-list density="compact" bg-color="transparent" class="pa-0">
                <v-list-item class="px-0 mb-4">
                  <template v-slot:prepend>
                    <v-icon color="zinc-muted" class="mr-3">mdi-folder-outline</v-icon>
                  </template>
                  <v-list-item-title class="text-zinc-primary font-weight-bold">
                    {{ t('sync.file_scanner') }}
                  </v-list-item-title>
                  <v-list-item-subtitle class="text-zinc-secondary">
                    {{ t('sync.idle') }}
                  </v-list-item-subtitle>
                </v-list-item>
                <v-list-item class="px-0 mb-4">
                  <template v-slot:prepend>
                    <v-icon color="zinc-muted" class="mr-3">mdi-auto-fix</v-icon>
                  </template>
                  <v-list-item-title class="text-zinc-primary font-weight-bold">
                    {{ t('sync.ai_intelligence') }}
                  </v-list-item-title>
                  <v-list-item-subtitle class="text-zinc-secondary">
                    {{ scanStore.indexingCount > 0
                      ? t('sync.jobs_remaining', { count: formatIndexingCount(scanStore.indexingCount) })
                      : t('sync.all_indexed')
                    }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-list>
              <v-divider class="my-4 border-subtle" />
              <v-btn
                v-if="!isActive"
                variant="flat"
                color="black"
                block
                height="56"
                class="siegu-btn"
                @click="emit('scan')"
              >
                <div class="d-flex align-center">
                  <div class="siegu-icon-circle mr-3">
                    <v-icon>mdi-sync</v-icon>
                  </div>
                  <div class="text-left">
                    <div class="font-weight-bold">{{ t('sync.sync_library') }}</div>
                    <div class="text-caption text-zinc-muted" style="font-size: 10px; opacity: 0.7">
                      {{ t('sync.refresh_files') }}
                    </div>
                  </div>
                </div>
              </v-btn>
              <div v-else class="text-center py-2">
                <v-progress-circular indeterminate color="black" size="24" />
                <div class="text-caption mt-2 text-zinc-muted">{{ t('sync.processing_bg') }}</div>
              </div>
            </v-card-text>
          </v-card>
        </v-menu>
      </v-col>

      <v-col class="mx-2 flex-grow-1">
        <div class="search-wrapper">
          <v-autocomplete
            v-model="selectedSearch"
            v-model:search="searchStore.query"
            :items="searchStore.tags"
            item-title="title"
            item-value="title"
            prepend-inner-icon="mdi-magnify"
            variant="solo-filled"
            density="compact"
            :placeholder="t('search.placeholder')"
            hide-details
            flat
            rounded="lg"
            class="search-autocomplete w-100"
            data-tour="search"
            bg-color="rgb(var(--v-theme-surface))"
            :menu-props="{ contentClass: 'siegu-list', elevation: 4 }"
            :no-data-text="!searchStore.tags.length ? t('search.no_data') : ''"
            :filter="() => true"
            return-object
            @keydown="handleSearchKeydown"
            @update:model-value="handleSearchSelect"
          >
            <template v-slot:item="{ props: itemProps, item }">
              <v-list-item v-bind="itemProps" :title="item.raw.title">
                <template v-slot:prepend>
                  <v-icon size="18" class="mr-2" :color="iconColor(item.raw.type)">
                    {{ iconForType(item.raw.type) }}
                  </v-icon>
                </template>
              </v-list-item>
            </template>
            <template v-slot:append-inner>
              <v-tooltip location="bottom" max-width="280">
                <template v-slot:activator="{ props: tooltipProps }">
                  <v-icon v-bind="tooltipProps" size="18" color="#a1a1aa" class="cursor-pointer">
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
            </template>
          </v-autocomplete>
        </div>
      </v-col>

      <v-col cols="auto">
        <v-menu :close-on-content-click="false" offset-y>
          <template v-slot:activator="{ props: filterProps }">
            <v-btn icon size="small" variant="text" v-bind="filterProps" color="#18181b">
              <v-badge :model-value="hasActiveFilters" color="black" dot px="1">
                <v-icon size="20">mdi-filter-variant</v-icon>
              </v-badge>
            </v-btn>
          </template>
          <v-card min-width="250" border class="mt-2 border-subtle" color="surface" rounded="xl">
            <v-list bg-color="transparent" density="compact" class="px-2 ga-2">
              <v-list-item class="px-0">
                <v-list-item-title class="text-zinc-secondary">
                  {{ t('filters.favorites_only') }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item class="px-0">
                <v-list-item-title class="text-zinc-secondary">
                  {{ t('filters.videos_only') }}
                </v-list-item-title>
              </v-list-item>
            </v-list>
          </v-card>
        </v-menu>
      </v-col>
    </v-row>
  </v-app-bar>
</template>

<style scoped>
.search-wrapper {
  width: 100%;
  cursor: text;
}
</style>
