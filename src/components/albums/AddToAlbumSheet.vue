<template>
  <v-bottom-sheet v-model="visible">
    <v-card class="rounded-t-xl pa-4" color="surface">
      <div class="d-flex align-center px-2 mb-2">
        <h3 class="text-h6 font-weight-bold text-zinc-primary">
          {{ $t('albums.add_to_album') }}
        </h3>
        <v-spacer></v-spacer>
        <v-btn icon variant="text" size="small" @click="visible = false">
          <v-icon size="20">mdi-close</v-icon>
        </v-btn>
      </div>

      <div class="d-flex ga-2 align-center px-2 mb-3">
        <v-text-field
          v-model="newName"
          :label="$t('albums.new_album_placeholder')"
          variant="outlined"
          density="comfortable"
          hide-details
          :disabled="creating"
          @keyup.enter="createAndAdd"
        ></v-text-field>
        <v-btn
          variant="flat"
          color="primary"
          :loading="creating"
          :disabled="!newName.trim()"
          class="siegu-btn-modern"
          @click="createAndAdd"
        >
          <v-icon start size="18">mdi-plus</v-icon>
          {{ $t('common.create') }}
        </v-btn>
      </div>

      <v-list density="compact" class="siegu-list" v-if="albums.length > 0">
        <v-list-item
          v-for="album in albums"
          :key="album.id"
          @click="addTo(album)"
          :prepend-icon="'mdi-folder-multiple-image-outline'"
        >
          <v-list-item-title>{{ album.name }}</v-list-item-title>
          <template v-slot:append>
            <v-icon size="20" color="var(--color-text-secondary)">mdi-plus-circle-outline</v-icon>
          </template>
        </v-list-item>
      </v-list>
      <p v-else class="text-caption text-zinc-muted px-2">{{ $t('albums.no_albums_yet') }}</p>
    </v-card>
  </v-bottom-sheet>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAlbumsStore } from '@/stores/albums'
import type { Album } from '@/types/albums'

const props = defineProps<{
  modelValue: boolean
  photoIds: string[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  added: [albumName: string]
}>()

const { t } = useI18n()
const albumsStore = useAlbumsStore()

const visible = ref(props.modelValue)
const newName = ref('')
const creating = ref(false)
const albums = ref<Album[]>(albumsStore.albums)

watch(
  () => props.modelValue,
  (value) => {
    visible.value = value
    if (value) {
      newName.value = ''
      void albumsStore.loadAlbums()
      albums.value = albumsStore.albums
    }
  },
)

watch(visible, (value) => emit('update:modelValue', value))

watch(
  () => albumsStore.albums,
  (value) => {
    albums.value = value
  },
)

async function addTo(album: Album): Promise<void> {
  visible.value = false
  await albumsStore.addItems(album.id, props.photoIds)
  emit('added', album.name)
}

async function createAndAdd(): Promise<void> {
  const name = newName.value.trim()
  if (!name || creating.value) return
  creating.value = true
  try {
    const album = await albumsStore.createAlbum(name)
    if (album) {
      visible.value = false
      await albumsStore.addItems(album.id, props.photoIds)
      emit('added', album.name)
    }
  } finally {
    creating.value = false
  }
}

void t
</script>
