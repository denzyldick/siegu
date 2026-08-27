<template>
  <v-card
    variant="flat"
    color="surface"
    rounded="xl"
    class="mb-6 overflow-hidden border"
    data-tour="settings-folders"
  >
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="surface" size="32" class="mr-3">
          <v-icon color="on-surface" size="small">mdi-folder-lock</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
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
                <v-icon color="rgba(var(--v-theme-on-surface), 0.6)" class="mr-2"
                  >mdi-folder</v-icon
                >
              </template>
              <v-list-item-title class="text-high-emphasis font-weight-medium text-truncate">{{
                directory.title
              }}</v-list-item-title>
              <v-list-item-subtitle class="text-disabled text-caption text-truncate">{{
                directory.value
              }}</v-list-item-subtitle>
              <template v-slot:append>
                <v-menu>
                  <template v-slot:activator="{ props }">
                    <v-btn
                      icon="mdi-dots-vertical"
                      variant="text"
                      size="small"
                      color="rgba(var(--v-theme-on-surface), 0.6)"
                      v-bind="props"
                    ></v-btn>
                  </template>
                  <v-list size="small">
                    <v-list-item @click="$emit('remove-directory', directory.value)">
                      <v-list-item-title>{{ $t('settings.remove_folder') }}</v-list-item-title>
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
              <v-divider v-if="index < directories.length - 1" class="border"></v-divider>
            </v-list-item>
          </v-list>
        </div>
        <div v-else class="text-center py-8 text-disabled border border-dashed rounded-lg border">
          <div>{{ $t('settings.no_folders') }}</div>
        </div>
      </v-expand-transition>
    </v-card-text>

    <v-card-actions style="background: rgb(var(--v-theme-surface))" class="pa-4 border-t">
      <v-btn
        variant="flat"
        color="primary"
        @click="$emit('select-directory')"
        block
        height="48"
        class="rounded-xl"
        data-tour="settings-folders-add"
      >
        <div class="d-flex align-center">
          <v-avatar color="rgba(255,255,255,0.2)" size="22" class="mr-2">
            <v-icon size="14" color="rgb(var(--v-theme-on-primary))">mdi-folder-plus</v-icon>
          </v-avatar>
          <span class="font-weight-bold">{{ $t('settings.add_folder') }}</span>
        </div>
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import type { DirectoryEntry } from '@/types/settings';

defineProps<{
  directories: DirectoryEntry[];
}>();

defineEmits<{
  'select-directory': [];
  'remove-directory': [path: string];
  'remove-directory-full': [path: string];
}>();
</script>
