import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { getIndexingStatus, getUnindexedCount } from '@/services/tauri'
import { listenEvent } from '@/services/events'
import { normalizeIndexingCount } from '@/composables/useMediaUtils'
import type { ScanStatus } from '@/types/scan'
import type { AiJob } from '@/types/models'

export const useScanStore = defineStore('scan', () => {
  const status = ref<ScanStatus>('idle')
  const scanning = ref(false)
  const filesFound = ref(0)
  const filesProcessed = ref(0)
  const currentFile = ref<string | null>(null)
  const aiJob = ref<AiJob | null>(null)
  const indexingCount = ref(0)
  const unindexedCount = ref(0)
  const indexingEta = ref<number | null>(null)

  const isActive = computed(() => scanning.value || status.value === 'indexing')
  const progress = computed(() => {
    if (filesFound.value === 0) return 0
    return Math.round((filesProcessed.value / filesFound.value) * 100)
  })

  async function loadIndexingStatus(): Promise<void> {
    try {
      const count = await getIndexingStatus()
      indexingCount.value = normalizeIndexingCount(count)
    } catch (error) {
      console.error('[ScanStore] Failed to load indexing status:', error)
    }
  }

  async function loadUnindexedCount(): Promise<void> {
    try {
      const count = await getUnindexedCount()
      unindexedCount.value = normalizeIndexingCount(count)
    } catch (error) {
      console.error('[ScanStore] Failed to load unindexed count:', error)
    }
  }

  void listenEvent('scan-progress', (payload) => {
    status.value = 'scanning'
    scanning.value = true
    filesFound.value = payload.files_found
    filesProcessed.value = payload.files_processed
    currentFile.value = payload.current_file

    if (payload.files_processed >= payload.files_found && payload.files_found > 0) {
      status.value = 'completed'
      scanning.value = false
      void loadIndexingStatus()
      void loadUnindexedCount()
    }
  })

  void listenEvent('indexing-progress', (payload) => {
    status.value = 'indexing'
    scanning.value = false
    indexingCount.value = normalizeIndexingCount(payload.completed)
  })

  void listenEvent('indexing-eta', (payload) => {
    indexingEta.value = payload.eta
  })

  void listenEvent('current-ai-job', (payload) => {
    aiJob.value = payload
  })

  return {
    status,
    scanning,
    filesFound,
    filesProcessed,
    currentFile,
    aiJob,
    indexingCount,
    unindexedCount,
    indexingEta,
    isActive,
    progress,
    loadIndexingStatus,
    loadUnindexedCount,
  }
})
