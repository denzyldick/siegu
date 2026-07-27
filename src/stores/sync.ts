import { defineStore } from 'pinia'
import { ref } from 'vue'
import { listDevices } from '@/services/tauri'
import { listenEvent } from '@/services/events'
import type { SyncStatus, Device } from '@/types/sync'

export const useSyncStore = defineStore('sync', () => {
  const status = ref<SyncStatus>('idle')
  const progress = ref({ total: 0, received: 0, current_file: null as string | null })
  const error = ref<string | null>(null)
  const devices = ref<Device[]>([])
  const connected = ref(false)
  const connectionMode = ref<'lan' | 'internet' | null>(null)

  async function loadDevices(): Promise<void> {
    try {
      const rawDevices = await listDevices()
      devices.value = rawDevices.map((d) => ({
        id: d.title,
        name: d.title,
        photo_count: d.photo_count,
        last_seen: d.subtitle,
      }))
    } catch (e) {
      console.error('[SyncStore] Failed to load devices:', e)
      devices.value = []
    }
  }

  void listenEvent('sync-progress', (payload) => {
    status.value = 'syncing'
    progress.value = {
      total: payload.total,
      received: payload.received,
      current_file: payload.current_file,
    }

    if (payload.received >= payload.total && payload.total > 0) {
      status.value = 'completed'
      connected.value = true
    }
  })

  void listenEvent('sync-error', (payload) => {
    status.value = 'error'
    error.value = payload.message
    connected.value = false
  })

  return {
    status,
    progress,
    error,
    devices,
    connected,
    connectionMode,
    loadDevices,
  }
})
