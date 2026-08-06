<template>
  <v-card
    variant="flat"
    color="surface"
    rounded="xl"
    class="mb-6 overflow-hidden border-subtle"
  >
    <v-card-item class="bg-zinc-100 py-4">
      <template v-slot:prepend>
        <div class="siegu-icon-circle-dark mr-3">
          <v-icon color="var(--color-text-btn)" size="small">mdi-folder-lock</v-icon>
        </div>
      </template>
      <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
        $t('settings.folders')
      }}</v-card-title>
    </v-card-item>

    <v-card-text class="pt-4">
      <v-expand-transition>
        <div v-if="directories.length > 0">
          <v-list bg-color="transparent">
            <v-list-item
              v-for="(directory, index) in directories"
              :key="directory.value"
              class="px-0"
            >
              <template v-slot:prepend>
                <v-icon color="var(--color-text-muted)" class="mr-2">mdi-folder</v-icon>
              </template>
              <v-list-item-title class="text-zinc-primary font-weight-medium text-truncate">{{
                directory.title
              }}</v-list-item-title>
              <v-list-item-subtitle class="text-zinc-muted text-caption text-truncate">{{
                directory.value
              }}</v-list-item-subtitle>
              <template v-slot:append>
                <v-menu>
                  <template v-slot:activator="{ props }">
                    <v-btn
                      icon="mdi-dots-vertical"
                      variant="text"
                      size="small"
                      color="var(--color-text-muted)"
                      v-bind="props"
                    ></v-btn>
                  </template>
                  <v-list size="small" class="siegu-list">
                    <v-list-item @click="$emit('remove-directory', directory.value)">
                      <v-list-item-title>{{
                        $t('settings.remove_folder')
                      }}</v-list-item-title>
                    </v-list-item>
                    <v-list-item
                      @click="$emit('remove-directory-full', directory.value)"
                      color="error"
                    >
                      <v-list-item-title>{{ $t('settings.wipe_folder') }}</v-list-item-title>
                    </v-list-item>
                  </v-list>
                </v-menu>
              </template>
              <v-divider
                v-if="index < directories.length - 1"
                class="border-subtle"
              ></v-divider>
            </v-list-item>
          </v-list>
        </div>
        <div
          v-else
          class="text-center py-8 text-zinc-muted border border-dashed rounded-lg border-subtle"
        >
          <div>{{ $t('settings.no_folders') }}</div>
        </div>
      </v-expand-transition>
    </v-card-text>

    <v-card-actions class="pa-4 bg-zinc-50 border-top-subtle">
      <v-btn
        variant="flat"
        theme="dark"
        @click="$emit('select-directory')"
        block
        height="48"
        class="siegu-btn rounded-xl"
      >
        <div class="d-flex align-center">
          <div class="siegu-icon-circle siegu-icon-circle-sm mr-2">
            <v-icon size="14" color="var(--color-text-btn)">mdi-folder-plus</v-icon>
          </div>
          <span class="font-weight-bold">{{ $t('settings.add_folder') }}</span>
        </div>
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import type { DirectoryEntry } from '@/types/settings'

defineProps<{
  directories: DirectoryEntry[]
}>()

defineEmits<{
  'select-directory': []
  'remove-directory': [path: string]
  'remove-directory-full': [path: string]
}>()
</script>
