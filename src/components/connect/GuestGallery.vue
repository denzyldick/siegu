<template>
  <div class="w-100">
    <div v-if="sync.viewOnlyLoading" class="text-center py-10">
      <v-progress-circular indeterminate color="primary" />
      <div class="text-body-2 text-medium-emphasis mt-3">
        {{ $t('connect.view_only_loading') }}
      </div>
    </div>

    <template v-else>
      <div class="d-flex align-center justify-space-between px-2 mb-2">
        <span class="text-caption text-medium-emphasis">
          {{ $t('connect.view_only_count', { count: sync.viewOnlyPhotos.length }) }}
        </span>
        <v-btn
          variant="text"
          size="small"
          color="primary"
          class="text-none"
          @click="sync.exitViewOnly()"
        >
          {{ $t('connect.view_only_exit') }}
        </v-btn>
      </div>

      <div v-if="sync.viewOnlyPhotos.length === 0" class="text-center py-8">
        <v-icon size="32" class="text-medium-emphasis mb-2">mdi-eye-off-outline</v-icon>
        <div class="text-body-2 text-medium-emphasis">{{ $t('connect.view_only_empty') }}</div>
      </div>

      <div v-else class="guest-grid">
        <button
          v-for="photo in sync.viewOnlyPhotos"
          :key="photo.id"
          type="button"
          class="guest-item"
          @click="openPreview(photo)"
        >
          <img
            :src="remoteThumbUrl(photo.id)"
            :alt="photo.caption ?? ''"
            loading="lazy"
            class="guest-thumb"
          />
          <v-icon v-if="isVideo(photo.location)" size="16" color="white" class="guest-video-badge">
            mdi-play
          </v-icon>
        </button>
      </div>
    </template>

    <v-dialog v-model="previewOpen" max-width="900" content-class="guest-preview-dialog">
      <v-card v-if="previewItem" dark class="bg-black">
        <v-toolbar density="compact" color="transparent">
          <v-toolbar-title class="text-body-2">
            {{ previewItem.caption ?? previewItem.location }}
          </v-toolbar-title>
          <v-spacer />
          <v-btn icon="mdi-close" variant="text" @click="previewOpen = false" />
        </v-toolbar>
        <div class="d-flex justify-center pa-2">
          <img
            v-if="!isVideo(previewItem.location)"
            :src="remoteMediaUrl(previewItem.id)"
            :alt="previewItem.caption ?? ''"
            class="guest-full"
          />
          <video
            v-else
            :src="remoteMediaUrl(previewItem.id)"
            controls
            playsinline
            preload="metadata"
            class="guest-full"
          />
        </div>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useSyncStore } from '@/stores/sync';
import { getMediaServerPort } from '@/services/tauri';
import { isVideoFile } from '@/types/media';
import type { ViewPhoto } from '@/types/sync';

const sync = useSyncStore();
const previewOpen = ref(false);
const previewItem = ref<ViewPhoto | null>(null);
let port: number | null = null;

void getMediaServerPort().then((p) => {
  port = p;
});

function remoteThumbUrl(id: string): string {
  return port ? `http://127.0.0.1:${port}/remote/thumb:${encodeURIComponent(id)}` : '';
}

function remoteMediaUrl(id: string): string {
  return port ? `http://127.0.0.1:${port}/remote/${encodeURIComponent(id)}` : '';
}

function isVideo(location: string): boolean {
  return isVideoFile(location);
}

function openPreview(photo: ViewPhoto): void {
  previewItem.value = photo;
  previewOpen.value = true;
}
</script>

<style scoped>
.guest-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
  gap: 4px;
}

.guest-item {
  position: relative;
  aspect-ratio: 1;
  border: none;
  padding: 0;
  background: rgb(var(--v-theme-surface-variant));
  border-radius: 4px;
  overflow: hidden;
  cursor: pointer;
}

.guest-thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.guest-video-badge {
  position: absolute;
  right: 4px;
  bottom: 4px;
  background: rgba(0, 0, 0, 0.55);
  border-radius: 50%;
  padding: 2px;
}

.guest-full {
  max-width: 100%;
  max-height: 75vh;
  object-fit: contain;
}
</style>
