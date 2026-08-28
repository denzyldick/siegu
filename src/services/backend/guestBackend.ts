/**
 * Guest-mode `Backend` adapter: routes data ops and media over the WebRTC RPC
 * client in `guest.ts` (#19, Phase 1).
 */
import type { GuestClient } from './guest';
import type { Backend } from './interface';
import { mediaCacheKey } from './interface';

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

    mediaUrl: (id, kind) => (kind === 'thumb' ? client.fetchThumb(id) : client.fetchOriginal(id)),
    cachedMediaUrl: (id, kind) => client.cachedUrl(mediaCacheKey(id, kind)),

    close: () => client.close(),
  };
}
