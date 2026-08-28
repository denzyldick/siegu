/**
 * Local Tauri `Backend` adapter: delegates data ops to the existing `tauri.ts`
 * commands and resolves media through the local media-server HTTP routes
 * (#19, Phase 1).
 */
import {
  getPhotoById,
  listFiles,
  searchFacets,
  countTrash,
  listTrash,
  toggleFavorite,
  trashPhoto,
  restorePhoto,
  emptyTrash,
} from '@/services/tauri';
import { useMediaUrl } from '@/composables/useMediaUrl';
import type { Backend, MediaKind } from './interface';
import { mediaCacheKey } from './interface';

export function tauriBackend(): Backend {
  const { ensurePort, thumbUrl, imageUrl } = useMediaUrl();
  const mediaCache = new Map<string, string>();

  return {
    listFiles,
    getPhotoById,
    searchFacets,
    countTrash,
    listTrash,
    toggleFavorite,
    trashPhoto,
    restorePhoto,
    emptyTrash: async () => {
      const n = await emptyTrash();
      mediaCache.clear();
      return n;
    },

    mediaUrl: async (id, kind: MediaKind) => {
      const key = mediaCacheKey(id, kind);
      const cached = mediaCache.get(key);
      if (cached) return cached;
      await ensurePort();
      const photo = await getPhotoById(Number(id));
      if (!photo) return null;
      const url = kind === 'thumb' ? thumbUrl(photo.location) : imageUrl(photo.location);
      if (url) mediaCache.set(key, url);
      return url;
    },
    cachedMediaUrl: (id, kind) => mediaCache.get(mediaCacheKey(id, kind)),

    close: () => mediaCache.clear(),
  };
}
