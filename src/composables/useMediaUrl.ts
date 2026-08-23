import { ref } from 'vue';
import { getMediaServerPort } from '@/services/tauri';
import { isVideoFile } from '@/types/media';

const sharedPort = ref<number | null>(null);
let portPromise: Promise<number | null> | null = null;

async function ensurePort(): Promise<number | null> {
  if (sharedPort.value !== null) return sharedPort.value;
  if (portPromise !== null) return portPromise;

  portPromise = (async () => {
    try {
      sharedPort.value = (await getMediaServerPort()) ?? null;
      return sharedPort.value;
    } catch (error) {
      console.error('[useMediaUrl] Failed to get media server port:', error);
      portPromise = null;
      return null;
    }
  })();

  return portPromise;
}

export function useMediaUrl() {
  void ensurePort();

  function videoUrl(location: string): string | null {
    if (!sharedPort.value || !location) return null;
    const encoded = encodeURIComponent(location);
    return `http://127.0.0.1:${sharedPort.value}/media/${encoded}`;
  }

  function imageUrl(location: string): string | null {
    if (!sharedPort.value || !location) return null;
    const encoded = encodeURIComponent(location);
    return `http://127.0.0.1:${sharedPort.value}/media/${encoded}`;
  }

  function thumbUrl(location: string): string | null {
    if (!sharedPort.value || !location) return null;
    const encoded = encodeURIComponent(location);
    return `http://127.0.0.1:${sharedPort.value}/thumb/${encoded}`;
  }

  // Evicted (view-only) items stream through the media server's /remote
  // route, which pulls bytes from the peer on demand (#10).
  function remoteImageUrl(id: string | number): string | null {
    if (!sharedPort.value) return null;
    return `http://127.0.0.1:${sharedPort.value}/remote/${encodeURIComponent(String(id))}`;
  }

  function remoteThumbUrl(id: string | number): string | null {
    if (!sharedPort.value) return null;
    return `http://127.0.0.1:${sharedPort.value}/remote/thumb:${encodeURIComponent(String(id))}`;
  }

  function isVideo(location: string): boolean {
    return isVideoFile(location);
  }

  return {
    port: sharedPort,
    ensurePort,
    videoUrl,
    imageUrl,
    thumbUrl,
    remoteImageUrl,
    remoteThumbUrl,
    isVideo,
  };
}
