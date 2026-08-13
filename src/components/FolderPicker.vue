<template>
  <v-dialog v-model="show" fullscreen transition="dialog-bottom-transition">
    <v-card rounded="0" color="background">
      <v-toolbar color="surface" border="bottom">
        <v-btn icon @click="close" color="primary">
          <v-icon>mdi-close</v-icon>
        </v-btn>
        <v-toolbar-title class="text-high-emphasis font-weight-bold">{{
          $t('folder_picker.select_folder')
        }}</v-toolbar-title>
        <v-spacer></v-spacer>
        <v-btn variant="flat" @click="selectCurrent" class="px-4 mr-2" color="primary">
          {{ $t('folder_picker.select_folder') }}
        </v-btn>
      </v-toolbar>

      <v-card-text class="pa-0">
        <div class="pa-4 border-bottom">
          <div class="text-caption text-medium-emphasis mb-1">
            {{ $t('folder_picker.current_path') }}
          </div>
          <div class="text-subtitle-1 font-weight-medium text-truncate">{{ currentPath }}</div>
        </div>

        <v-list v-if="loading">
          <v-list-item>
            <div class="d-flex justify-center align-center py-8">
              <v-progress-circular indeterminate color="primary"></v-progress-circular>
            </div>
          </v-list-item>
        </v-list>

        <v-list v-else lines="one">
          <v-list-item v-if="canGoUp" @click="goUp" color="primary">
            <template v-slot:prepend>
              <v-icon color="grey-darken-1">mdi-arrow-up-bold</v-icon>
            </template>
            <v-list-item-title>..</v-list-item-title>
            <v-list-item-subtitle>{{ $t('folder_picker.go_up') }}</v-list-item-subtitle>
          </v-list-item>

          <v-divider v-if="canGoUp"></v-divider>

          <template v-if="folders.length > 0">
            <v-list-item
              v-for="folder in folders"
              :key="folder.name"
              @click="navigate(folder.name)"
              ripple
            >
              <template v-slot:prepend>
                <v-icon color="amber-darken-2">mdi-folder</v-icon>
              </template>
              <v-list-item-title>{{ folder.name }}</v-list-item-title>
            </v-list-item>
          </template>

          <div v-else class="text-center py-8 text-medium-emphasis">
            <v-icon color="grey-lighten-1" size="large" class="mb-2"
              >mdi-folder-open-outline</v-icon
            >
            <div>{{ $t('folder_picker.no_subfolders') }}</div>
          </div>
        </v-list>
      </v-card-text>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { readDir } from '@tauri-apps/plugin-fs';
import type { DirEntry } from '@tauri-apps/plugin-fs';

const { t } = useI18n();

const props = defineProps<{
  modelValue: boolean;
  initialPath?: string;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
  select: [path: string];
}>();

const DEFAULT_PATH = '/storage/emulated/0';

const currentPath = ref(props.initialPath ?? DEFAULT_PATH);
const folders = ref<DirEntry[]>([]);
const loading = ref(false);

const show = computed({
  get: () => props.modelValue,
  set: (val: boolean) => emit('update:modelValue', val),
});

const canGoUp = computed(() => {
  return currentPath.value !== '/' && currentPath.value !== DEFAULT_PATH;
});

watch(show, (val) => {
  if (val) {
    loadDirectory(currentPath.value);
  }
});

onMounted(() => {
  if (props.initialPath) {
    currentPath.value = props.initialPath;
  }
});

async function loadDirectory(path: string) {
  loading.value = true;
  try {
    const entries = await readDir(path);
    folders.value = entries
      .filter(
        (entry): entry is DirEntry & { isDirectory: true; name: string } =>
          entry.isDirectory === true && entry.name != null,
      )
      .sort((a, b) => a.name.localeCompare(b.name));
    currentPath.value = path;
  } catch {
    alert(t('folder_picker.permission_error'));
  } finally {
    loading.value = false;
  }
}

async function navigate(folderName: string) {
  const newPath = currentPath.value.endsWith('/')
    ? currentPath.value + folderName
    : currentPath.value + '/' + folderName;
  await loadDirectory(newPath);
}

async function goUp() {
  const parts = currentPath.value.split('/');
  if (parts.length > 1) {
    parts.pop();
    let newPath = parts.join('/');
    if (newPath === '') newPath = '/';
    await loadDirectory(newPath);
  }
}

function selectCurrent() {
  emit('select', currentPath.value);
  close();
}

function close() {
  emit('update:modelValue', false);
}
</script>
