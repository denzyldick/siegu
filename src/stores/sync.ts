import { defineStore } from 'pinia'
import { ref } from 'vue'
import { listDevices } from '@/services/tauri'
import { listenEvent } from '@/services/events'
import type { SyncStatus, Device } from '@/types/sync'

export const useSyncStore = defineStore('sync', () => {
  const status = ref<SyncStatus>('idle')
  const progress = ref({ items_completed: 0, items_total: 0, status: '' })
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

  void listenEvent('sync-progress', (payload: {
    device_id: string
    status: string
    progress: number
    bytes_per_second: number
    items_completed: number
    items_total: number
  }) => {
    progress.value = {
      items_completed: payload.items_completed ?? 0,
      items_total: payload.items_total ?? 0,
      status: payload.status,
    }
    if (payload.status.toLowerCase().includes('syncing')) {
      status.value = 'syncing'
    }
    if (payload.status === 'Up to date' || payload.status.startsWith('Finished')) {
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
