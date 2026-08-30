/**
 * Guest-mode `Backend` adapter: routes data ops and media over the WebRTC RPC
 * client in `guest.ts` (#19, Phase 1).
 */
import type { GuestClient } from './guest';
import type { Backend } from './interface';
import { mediaCacheKey } from './interface';
import type { MediaItem } from '@/types/media';

export function guestBackend(client: GuestClient): Backend {
  return {
    listFiles: (options) => client.listFiles(options),
    getPhotoById: (id) => client.getPhotoById(id),
    searchFacets: () => client.searchFacets(),
    countTrash: () => client.countTrash(),
    listTrash: (limit = 100) => client.listTrash(limit),
    toggleFavorite: (id) => client.toggleFavorite(id),
    trashPhoto: (id) => client.trashPhoto(id),
    restorePhoto: (id) => client.restorePhoto(id),
    emptyTrash: () => client.emptyTrash(),

    getPhotosByIds: async (ids) => {
      const out: MediaItem[] = [];
      for (const id of ids) {
        const item = await client.getPhotoById(id);
        if (item) out.push(item);
      }
      return out;
    },
    getPhotoEncodedBatch: async (ids) => {
      const out: Record<number, string> = {};
      for (const id of ids) {
        const thumb = await client.fetchThumb(id);
        if (thumb) out[Number(id)] = thumb;
      }
      return out;
    },
    setFavorites: async (ids, favorite) => {
      for (const id of ids) {
        const cur = await client.getPhotoById(id);
        if (cur && cur.favorite !== favorite) await client.toggleFavorite(id);
      }
      return ids.length;
    },

    request: (name, payload = {}) => client.request(name, payload),

    mediaUrl: (id, kind) => (kind === 'thumb' ? client.fetchThumb(id) : client.fetchOriginal(id)),
    cachedMediaUrl: (id, kind) => client.cachedUrl(mediaCacheKey(id, kind)),

    close: () => client.close(),
  };
}
