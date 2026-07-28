<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { discoverLanDevices } from '@/services/tauri'
import type { DiscoveredHost } from '@/types/sync'

const emit = defineEmits<{
  select: [host: DiscoveredHost]
}>()

const hosts = ref<DiscoveredHost[]>([])
const scanning = ref(true)
const elapsed = ref(0)
let pollTimer: ReturnType<typeof setInterval> | null = null
let elapsedTimer: ReturnType<typeof setInterval> | null = null

const showUpsell = computed(() => elapsed.value >= 5 && hosts.value.length === 0)

async function poll(): Promise<void> {
  try {
    const results = await discoverLanDevices(2)
    hosts.value = results
  } catch {
    // ignore poll errors
  } finally {
    scanning.value = false
  }
}

onMounted(() => {
  poll()
  pollTimer = setInterval(poll, 3000)
  elapsedTimer = setInterval(() => {
    elapsed.value++
  }, 1000)
})

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer)
  if (elapsedTimer) clearInterval(elapsedTimer)
})
</script>

<template>
  <div class="d-flex flex-column ga-3 w-100">
    <div class="text-caption text-zinc-muted text-center uppercase tracking-widest mb-2">
      {{ $t('connect.select_device') }}
    </div>

    <div v-if="scanning && hosts.length === 0" class="d-flex align-center justify-center py-6 ga-3">
      <v-progress-circular indeterminate color="black" size="20" width="2" />
      <span class="text-caption text-zinc-secondary">{{ $t('connect.searching_network') }}</span>
    </div>

    <v-list v-if="hosts.length > 0" density="compact" class="bg-transparent pa-0">
      <v-list-item
        v-for="host in hosts"
        :key="`${host.ip}:${host.port}`"
        @click="emit('select', host)"
        rounded="lg"
        class="mb-1 border-subtle"
      >
        <template v-slot:prepend>
          <v-icon size="20" color="black">mdi-laptop</v-icon>
        </template>
        <v-list-item-title class="text-body-2 font-weight-medium">{{ host.name }}</v-list-item-title>
        <v-list-item-subtitle class="text-caption">{{ host.ip }}</v-list-item-subtitle>
        <template v-slot:append>
          <v-icon size="16" color="grey">mdi-chevron-right</v-icon>
        </template>
      </v-list-item>
    </v-list>

    <div
      v-if="!scanning && hosts.length === 0"
      class="text-caption text-zinc-muted text-center py-4"
    >
      {{ $t('connect.no_devices_found') }}
    </div>

    <v-card
      v-if="showUpsell"
      variant="flat"
      class="bg-zinc-50 border-subtle rounded-xl pa-4 text-center mt-2"
    >
      <v-icon size="24" color="grey" class="mb-2">mdi-wifi-off</v-icon>
      <div class="text-body-2 text-zinc-secondary">
        {{ $t('connect.connect_fromAnywhere') }}
      </div>
    </v-card>
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
