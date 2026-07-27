import { ref } from 'vue'
import { getMediaServerPort } from '@/services/tauri'
import { isVideoFile } from '@/types/media'

const sharedPort = ref<number | null>(null)
let portPromise: Promise<number | null> | null = null

async function ensurePort(): Promise<number | null> {
  if (sharedPort.value !== null) return sharedPort.value
  if (portPromise !== null) return portPromise

  portPromise = (async () => {
    try {
      const port = await getMediaServerPort()
      sharedPort.value = port
      return port
    } catch (error) {
      console.error('[useMediaUrl] Failed to get media server port:', error)
      return null
    }
  })()

  return portPromise
}

export function useMediaUrl() {
  function videoUrl(location: string): string | null {
    if (!sharedPort.value || !location) return null
    const encoded = encodeURIComponent(location)
    return `http://127.0.0.1:${sharedPort.value}/media/${encoded}`
  }

  function imageUrl(location: string): string | null {
    if (!sharedPort.value || !location) return null
    const encoded = encodeURIComponent(location)
    return `http://127.0.0.1:${sharedPort.value}/media/${encoded}`
  }

  function isVideo(location: string): boolean {
    return isVideoFile(location)
  }

  return {
    port: sharedPort,
    ensurePort,
    videoUrl,
    imageUrl,
    isVideo,
  }
}
