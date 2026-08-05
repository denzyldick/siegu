import { defineStore } from 'pinia'
import { ref } from 'vue'
import { checkModels, downloadModels, getConfig, saveConfig } from '@/services/tauri'
import { listenEvent } from '@/services/events'
import type { ModelProgress } from '@/types/models'

function configKeyForModel(modelId: string): string {
  return 'model_enabled_' + modelId
}

function configKeysForModel(modelId: string): string[] {
  if (modelId === 'face') {
    return ['model_enabled_face', 'model_enabled_arcface']
  }
  return ['model_enabled_' + modelId]
}

export const useModelsStore = defineStore('models', () => {
  const downloaded = ref<string[]>([])
  const downloading = ref(false)
  const downloadProgress = ref<Record<string, number>>({})
  const modelProgress = ref<ModelProgress | null>(null)
  const enabled = ref<Record<string, boolean>>({})

  async function loadModels(): Promise<void> {
    try {
      downloaded.value = await checkModels()
      const config = await getConfig()
      for (const model of downloaded.value) {
        const key = configKeyForModel(model)
        enabled.value[model] = config[key] !== 'false'
      }
    } catch (error) {
      console.error('[ModelsStore] Failed to load models:', error)
    }
  }

  async function downloadSelectedModels(models: string[]): Promise<void> {
    if (models.length === 0) return
    downloading.value = true
    downloadProgress.value = {}
    try {
      await downloadModels(models)
    } catch (error) {
      console.error('[ModelsStore] Failed to download models:', error)
    } finally {
      downloading.value = false
    }
  }

  async function toggleModel(modelId: string): Promise<void> {
    enabled.value[modelId] = !enabled.value[modelId]
    try {
      for (const key of configKeysForModel(modelId)) {
        await saveConfig(key, enabled.value[modelId] ? 'true' : 'false')
      }
    } catch (error) {
      console.error('[ModelsStore] Failed to save model state:', error)
    }
  }

  function isModelEnabled(modelId: string): boolean {
    return enabled.value[modelId] !== false
  }

  function isDownloaded(modelId: string): boolean {
    return downloaded.value.includes(modelId)
  }

  void listenEvent('download-progress', (payload) => {
    if (payload.total) {
      downloadProgress.value[payload.model] = Math.min(
        100,
        Math.round((payload.downloaded / payload.total) * 100),
      )
    }
  })

  void listenEvent('model-progress', (payload) => {
    modelProgress.value = payload
  })

  return {
    downloaded,
    downloading,
    downloadProgress,
    modelProgress,
    enabled,
    loadModels,
    downloadSelectedModels,
    toggleModel,
    isModelEnabled,
    isDownloaded,
  }
})
