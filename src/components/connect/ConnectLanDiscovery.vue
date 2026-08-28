<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { discoverLanDevices, isAndroid, isTauri, pingMdnsPlugin } from '@/services/tauri';
import type { DiscoveredHost } from '@/types/sync';

const emit = defineEmits<{
  select: [host: DiscoveredHost];
}>();

const hosts = ref<DiscoveredHost[]>([]);
const scanning = ref(true);
const webOnly = ref(!isTauri);
const error = ref('');
const elapsed = ref(0);
let pollTimer: ReturnType<typeof setInterval> | null = null;
let elapsedTimer: ReturnType<typeof setInterval> | null = null;

const noDevices = computed(() => elapsed.value >= 15 && hosts.value.length === 0); // Show the spinner for the first 15s

async function initPlugin(): Promise<boolean> {
  if (webOnly.value) {
    // Plain browser build: mDNS LAN discovery is Tauri-only; stop here and
    // surface the web hint instead of scanning/hanging.
    error.value = '';
    scanning.value = false;
    return false;
  }
  if (isAndroid) {
    try {
      await pingMdnsPlugin();
      return true;
    } catch {
      error.value = 'mDNS plugin not available';
      scanning.value = false;
      return false;
    }
  }
  return true;
}

async function poll(): Promise<void> {
  try {
    const results = await discoverLanDevices(2);
    if (results.length > 0) {
      hosts.value = results;
      error.value = '';
      scanning.value = false;
    }
  } catch (e) {
    error.value = String(e);
  }
}

onMounted(async () => {
  const ok = await initPlugin();
  if (ok) {
    poll();
    pollTimer = setInterval(poll, 3000);
  }
  elapsedTimer = setInterval(() => {
    elapsed.value++;
  }, 1000);
});

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
  if (elapsedTimer) clearInterval(elapsedTimer);
});
</script>

<template>
  <div class="d-flex flex-column ga-3 w-100">
    <div class="text-caption text-disabled text-center uppercase tracking-widest mb-2">
      {{ $t('connect.select_device') }}
    </div>

    <div v-if="webOnly" class="text-caption text-medium-emphasis text-center py-4 px-2">
      {{ $t('connect.web_enter_host_hint') }}
    </div>

    <div
      v-else-if="scanning && hosts.length === 0"
      class="d-flex align-center justify-center py-6 ga-3"
    >
      <v-progress-circular
        indeterminate
        color="rgba(var(--v-theme-on-surface), 0.7)"
        size="20"
        width="2"
      />
      <span class="text-caption text-medium-emphasis">{{ $t('connect.searching_network') }}</span>
    </div>

    <v-list v-if="hosts.length > 0" density="compact" class="bg-transparent pa-0 w-100">
      <v-list-item
        v-for="host in hosts"
        :key="`${host.ip}:${host.port}`"
        @click="emit('select', host)"
        rounded="lg"
        class="mb-1 border"
      >
        <template v-slot:prepend>
          <v-icon size="20" color="rgba(var(--v-theme-on-surface), 0.7)">mdi-laptop</v-icon>
        </template>
        <v-list-item-title class="text-body-2 font-weight-medium">{{
          host.name || host.ip
        }}</v-list-item-title>
        <template v-slot:append>
          <v-icon size="16" color="grey">mdi-chevron-right</v-icon>
        </template>
      </v-list-item>
    </v-list>

    <div v-if="noDevices" class="text-caption text-disabled text-center py-4">
      {{ error || $t('connect.no_devices_found') }}
    </div>
  </div>
</template>

<style scoped>
.uppercase {
  text-transform: uppercase;
}

.tracking-widest {
  letter-spacing: 0.1em;
}
</style>
