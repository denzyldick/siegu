<template>
  <v-container class="pa-6">
    <!-- Header -->
    <div class="d-flex align-center justify-space-between mb-6 flex-wrap ga-3">
      <div>
        <div class="d-flex align-center mb-1">
          <v-icon color="rgb(var(--v-theme-on-surface))" size="28" class="mr-3"
            >mdi-crop-free</v-icon
          >
          <h1 class="text-h4 font-weight-bold text-high-emphasis">{{ $t('duplicates.title') }}</h1>
        </div>
        <div class="text-subtitle-1 text-medium-emphasis">{{ $t('duplicates.desc') }}</div>
      </div>
      <v-btn
        variant="tonal"
        prepend-icon="mdi-refresh"
        :loading="store.scanning"
        @click="store.startScan(true)"
      >
        {{ $t('duplicates.refresh') }}
      </v-btn>
    </div>

    <!-- Disk overview -->
    <v-card variant="flat" class="border pa-5 mb-4 storage-overview">
      <div class="d-flex align-center justify-space-between flex-wrap ga-4">
        <div>
          <div class="text-caption text-medium-emphasis">{{ $t('duplicates.storage_total') }}</div>
          <div class="text-h4 font-weight-bold text-high-emphasis storage-value">
            {{ hasOverview ? formatBytes(store.libraryBytes) : '—' }}
          </div>
        </div>
        <div>
          <div class="text-caption text-medium-emphasis">{{ $t('duplicates.reclaimable') }}</div>
          <div class="text-h4 font-weight-bold text-error storage-value">
            {{ hasStats ? formatBytes(reclaimable) : '—' }}
          </div>
        </div>
        <div>
          <div class="text-caption text-medium-emphasis">{{ $t('duplicates.contents') }}</div>
          <div class="text-h6 font-weight-bold text-high-emphasis">
            {{
              hasOverview
                ? $t('duplicates.media_count', {
                    photos: store.photoCount.toLocaleString(),
                    videos: store.videoCount.toLocaleString(),
                  })
                : '—'
            }}
          </div>
        </div>
      </div>

      <div class="mt-4">
        <v-progress-linear
          v-if="hasOverview && store.libraryBytes > 0"
          :model-value="reclaimShare"
          :height="10"
          rounded
          color="error"
          bg-color="rgba(var(--v-theme-on-surface), 0.12)"
        />
        <v-progress-linear
          v-else-if="store.scanning"
          indeterminate
          :height="10"
          rounded
          color="primary"
        />
        <div class="d-flex justify-space-between mt-1 text-caption">
          <span class="text-disabled">{{ $t('duplicates.in_use') }}</span>
          <span v-if="hasOverview && store.libraryBytes > 0" class="text-error font-weight-medium">
            {{ reclaimShareText }} {{ $t('duplicates.reclaim_share') }}
          </span>
          <span class="text-disabled">{{ $t('duplicates.reclaimable') }}</span>
        </div>
      </div>
    </v-card>

    <!-- Scan progress -->
    <div v-if="store.scanning" class="mb-4">
      <v-card variant="flat" class="border pa-4 d-flex align-center ga-4">
        <v-progress-circular indeterminate size="28" color="primary" />
        <div class="flex-grow-1">
          <div class="text-subtitle-1 font-weight-bold">{{ $t('duplicates.scanning') }}</div>
          <div v-if="store.progress.total > 0" class="text-caption text-medium-emphasis">
            {{
              $t('duplicates.scanning_count', {
                done: store.progress.done,
                total: store.progress.total,
              })
            }}
          </div>
          <div class="text-caption text-disabled">{{ $t('duplicates.scanning_hint') }}</div>
        </div>
        <div v-if="store.progress.total > 0" class="text-subtitle-1 font-weight-bold">
          {{ scanPercent }}
        </div>
      </v-card>
    </div>

    <!-- Empty State -->
    <div
      v-if="!store.scanning && (!store.stats || store.stats.group_count === 0)"
      class="d-flex flex-column align-center justify-center py-16 text-center animate-fade-in"
    >
      <v-icon size="64" color="rgba(var(--v-theme-on-surface), 0.25)" class="mb-4"
        >mdi-file-check-outline</v-icon
      >
      <div class="text-h6 text-medium-emphasis font-weight-bold">
        {{ $t('duplicates.no_duplicates') }}
      </div>
      <p class="text-body-2 text-disabled mt-1 max-w-400 mx-auto">
        {{ $t('duplicates.no_duplicates_desc') }}
      </p>
    </div>

    <!-- Groups -->
    <div v-else class="d-flex flex-column ga-4">
      <v-card
        v-for="(group, gi) in store.groups"
        :key="groupKey(group)"
        variant="flat"
        class="border pa-4"
      >
        <div class="d-flex align-center justify-space-between mb-3 flex-wrap ga-2">
          <div class="d-flex align-center ga-2 flex-wrap">
            <v-chip size="small" label color="primary" variant="tonal">
              {{ $t('duplicates.kind_' + group.kind) }}
            </v-chip>
            <span v-if="group.unknown_best" class="text-caption text-warning align-self-center ml-1">
              {{ $t('duplicates.unknown_best') }}
            </span>
          </div>
          <div class="d-flex align-center ga-2">
            <span class="text-caption text-medium-emphasis">
              {{ formatBytes(group.reclaimable_bytes) }}
            </span>
            <v-btn
              size="small"
              variant="tonal"
              color="error"
              prepend-icon="mdi-delete-outline"
              :loading="removingGroup === gi"
              @click="trashGroup(group, gi)"
            >
              {{ $t('duplicates.trash_duplicates', { count: group.members.length - 1 }) }}
            </v-btn>
          </div>
        </div>

        <div class="duplicate-strip">
          <div
            v-for="member in group.members"
            :key="member.id"
            class="duplicate-member"
            :class="{
              'duplicate-member--keep': isKeep(group, member.id),
              'duplicate-member--trash': !isKeep(group, member.id),
            }"
            role="button"
            tabindex="0"
            :aria-label="member.location"
            @click="selectKeep(group, member.id)"
            @keydown.enter="selectKeep(group, member.id)"
          >
            <DuplicateThumb :id="member.id" :location="member.location" />
            <div class="duplicate-member-badge" v-if="isKeep(group, member.id)">
              <v-icon size="14">mdi-check</v-icon>
            </div>
            <div class="duplicate-member-label">
              <span v-if="isKeep(group, member.id)" class="font-weight-bold text-primary">{{
                $t('duplicates.keep')
              }}</span>
              <span v-else-if="member.aesthetics != null" class="text-caption">{{
                member.aesthetics.toFixed(1)
              }}</span>
            </div>
          </div>
        </div>
      </v-card>
    </div>
  </v-container>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { formatBytes } from '@/utils/format';
import { useDuplicatesStore } from '@/stores/duplicates';
import DuplicateThumb from '@/components/duplicates/DuplicateThumb.vue';
import type { DuplicateGroupView } from '@/types/duplicates';

const store = useDuplicatesStore();

const keepSelections = ref<Record<string, string>>({});
const removingGroup = ref<number | null>(null);

const hasOverview = computed(() => store.libraryBytes > 0 || store.photoCount > 0);
const hasStats = computed(() => store.stats !== null);
const reclaimable = computed(() => store.stats?.reclaimable_bytes ?? 0);
const reclaimShare = computed(() =>
  store.libraryBytes > 0 ? Math.min(100, (reclaimable.value / store.libraryBytes) * 100) : 0,
);
const reclaimShareText = computed(() =>
  reclaimShare.value >= 10
    ? `${Math.round(reclaimShare.value)}%`
    : `${reclaimShare.value.toFixed(1)}%`,
);
const scanPercent = computed(() => {
  const p = store.progress;
  if (!p || p.total <= 0) return '';
  return `${Math.min(100, Math.round((p.done / p.total) * 100))}%`;
});

function groupKey(group: DuplicateGroupView): string {
  return group.members
    .map((m) => m.id)
    .sort()
    .join('|');
}

function effectiveKeep(group: DuplicateGroupView): string {
  return (
    keepSelections.value[groupKey(group)] ??
    group.best_id ??
    group.members[0]?.id
  );
}

function isKeep(group: DuplicateGroupView, id: string): boolean {
  return id === effectiveKeep(group);
}

function selectKeep(group: DuplicateGroupView, id: string): void {
  keepSelections.value[groupKey(group)] = id;
}

async function trashGroup(group: DuplicateGroupView, gi: number): Promise<void> {
  if (removingGroup.value !== null) return;
  const keep = effectiveKeep(group);
  removingGroup.value = gi;
  try {
    await store.trashGroup(gi, keep);
  } finally {
    removingGroup.value = null;
  }
}

// Reset transient per-scan state whenever a fresh scan starts or completes.
watch(
  () => store.scanning,
  (v) => {
    if (v) {
      keepSelections.value = {};
      removingGroup.value = null;
    }
  },
);
watch(
  () => store.stats,
  () => {
    keepSelections.value = {};
    removingGroup.value = null;
  },
);

onMounted(() => {
  keepSelections.value = {};
  void store.ensureLoaded();
});
</script>

<style scoped>
.storage-overview {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, rgb(var(--v-theme-primary)) 8%, transparent),
    transparent 60%
  );
}

.storage-value {
  letter-spacing: -0.01em;
}

.duplicate-strip {
  display: flex;
  gap: 12px;
  overflow-x: auto;
  padding-bottom: 4px;
}

.duplicate-member {
  position: relative;
  flex: 0 0 auto;
  width: 128px;
  height: 128px;
  cursor: pointer;
  border-radius: 12px;
  overflow: hidden;
  border: 2px solid rgba(var(--v-theme-on-surface), 0.12);
  transition:
    border-color 0.2s ease,
    transform 0.2s ease;
}

.duplicate-member:hover {
  transform: translateY(-2px);
}

.duplicate-member--keep {
  border-color: rgb(var(--v-theme-primary));
}

.duplicate-member--trash {
  opacity: 0.6;
}

.duplicate-member--trash:hover {
  opacity: 1;
}

.duplicate-member-badge {
  position: absolute;
  top: 6px;
  left: 6px;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  background: rgb(var(--v-theme-primary));
}

.duplicate-member-label {
  position: absolute;
  right: 6px;
  bottom: 6px;
  left: 6px;
  display: flex;
  justify-content: center;
  padding: 2px 6px;
  border-radius: 9999px;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 11px;
}
</style>