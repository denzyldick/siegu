<template>
  <v-dialog v-model="show" fullscreen transition="dialog-bottom-transition">
    <v-card rounded="0" color="#fafafa">
      <v-toolbar color="white" border="bottom">
        <v-btn icon @click="close" color="#18181b">
          <v-icon>mdi-close</v-icon>
        </v-btn>
        <v-toolbar-title class="text-zinc-primary font-weight-bold">{{ $t('folder_picker.select_folder') }}</v-toolbar-title>
        <v-spacer></v-spacer>
        <v-btn variant="flat" @click="selectCurrent" class="siegu-btn px-4 mr-2">
          {{ $t('folder_picker.select_folder') }}
        </v-btn>
      </v-toolbar>

      <v-card-text class="pa-0">
        <!-- Current Path Breadcrumb/Display -->
        <div class="pa-4 border-bottom">
            <div class="text-caption text-medium-emphasis mb-1">{{ $t('folder_picker.current_path') }}</div>
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
          <!-- Go Up Option -->
          <v-list-item v-if="canGoUp" @click="goUp" color="primary">
            <template v-slot:prepend>
              <v-icon color="grey-darken-1">mdi-arrow-up-bold</v-icon>
            </template>
            <v-list-item-title>..</v-list-item-title>
            <v-list-item-subtitle>{{ $t('folder_picker.go_up') }}</v-list-item-subtitle>
          </v-list-item>

          <v-divider v-if="canGoUp"></v-divider>

          <!-- Directories -->
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
            <v-icon color="grey-lighten-1" size="large" class="mb-2">mdi-folder-open-outline</v-icon>
            <div>{{ $t('folder_picker.no_subfolders') }}</div>
          </div>

        </v-list>
      </v-card-text>
    </v-card>
  </v-dialog>
</template>

<script>
import { readDir } from "@tauri-apps/plugin-fs";
import { join, dirname } from "@tauri-apps/api/path";

export default {
  props: {
    modelValue: Boolean,
    initialPath: {
        type: String,
        default: "/storage/emulated/0"
    }
  },
  data() {
    return {
      currentPath: "/storage/emulated/0",
      folders: [],
      loading: false,
      error: null,
    };
  },
  computed: {
    show: {
      get() {
        return this.modelValue;
      },
      set(val) {
        this.$emit("update:modelValue", val);
      }
    },
    canGoUp() {
        return this.currentPath !== "/" && this.currentPath !== "/storage/emulated/0";
    }
  },
  watch: {
    show(val) {
        if (val) {
            this.loadDirectory(this.currentPath);
        }
    }
  },
  async mounted() {
      // Set initial path if provided, else default to Android root
      if (this.initialPath) {
          this.currentPath = this.initialPath;
      }
  },
  methods: {
    async loadDirectory(path) {
      this.loading = true;
      this.error = null;
      try {
        console.log("Reading directory:", path);
        const entries = await readDir(path);

        // Filter only directories and sort them
        this.folders = entries
            .filter(entry => entry.isDirectory)
            .sort((a, b) => a.name.localeCompare(b.name));

        this.currentPath = path;
      } catch (err) {
        console.error("Error reading directory:", err);
        this.error = this.$t('folder_picker.access_denied');
        alert(this.$t('folder_picker.permission_error'));
      } finally {
        this.loading = false;
      }
    },
    async navigate(folderName) {
        const newPath = this.currentPath.endsWith('/')
            ? this.currentPath + folderName
            : this.currentPath + '/' + folderName;

        // Or better use join API if available and reliable,
        // but simple string concat is often safer for known Android paths
        // await join(this.currentPath, folderName);

        await this.loadDirectory(newPath);
    },
    async goUp() {
        // Simple string manipulation for path parent to avoid async mess with path module for now
        // if this.currentPath is /storage/emulated/0/DCIM, we want /storage/emulated/0
        const parts = this.currentPath.split('/');
        if (parts.length > 1) {
            parts.pop();
            // Handle root case if "pop" makes it empty string (should happen if path was /foo)
            let newPath = parts.join('/');
            if (newPath === "") newPath = "/";
            await this.loadDirectory(newPath);
        }
    },
    selectCurrent() {
      this.$emit("select", this.currentPath);
      this.close();
    },
    close() {
      this.$emit("update:modelValue", false);
    }
  }
};
</script>
