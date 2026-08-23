import { defineStore } from 'pinia';
import { ref } from 'vue';
import { listDevices, autoReconnect, enterViewOnly } from '@/services/tauri';
import { listenEvent } from '@/services/events';
import type { SyncStatus, Device, ViewPhoto } from '@/types/sync';

export const useSyncStore = defineStore('sync', () => {
  const status = ref<SyncStatus>('idle');
  const progress = ref({ items_completed: 0, items_total: 0, status: '' });
  const error = ref<string | null>(null);
  const currentFile = ref<{ filename: string; thumbnail: string } | null>(null);
  const devices = ref<Device[]>([]);
  const connected = ref(false);
  const connection = ref<'idle' | 'connected' | 'offline'>('idle');
  const connectionMode = ref<'lan' | 'internet' | null>(null);
  const activePeerId = ref<string | null>(null);
  /** #9 view-only browsing state. */
  const viewOnlyActive = ref(false);
  const viewOnlyPhotos = ref<ViewPhoto[]>([]);
  const viewOnlyLoading = ref(false);

  async function loadDevices(): Promise<void> {
    try {
      const rawDevices = await listDevices();
      devices.value = rawDevices.map((d) => ({
        id: d.id,
        name: d.title,
        photo_count: d.photo_count,
        video_count: d.video_count,
        remote_photo_count: d.remote_photo_count ?? 0,
        remote_video_count: d.remote_video_count ?? 0,
        os: d.os,
        icon: d.icon,
        host: d.host,
        last_seen: d.subtitle,
      }));
    } catch (e) {
      console.error('[SyncStore] Failed to load devices:', e);
      devices.value = [];
    }
  }

  void listenEvent(
    'sync-progress',
    (payload: {
      device_id: string;
      status: string;
      progress: number;
      bytes_per_second: number;
      items_completed: number;
      items_total: number;
      phase?: 'idle' | 'syncing' | 'completed';
      filename?: string;
      thumbnail?: string;
    }) => {
      progress.value = {
        items_completed: payload.items_completed ?? 0,
        items_total: payload.items_total ?? 0,
        status: payload.status,
      };
      if (payload.filename && payload.thumbnail) {
        currentFile.value = { filename: payload.filename, thumbnail: payload.thumbnail };
      }
      if (payload.phase === 'completed' && !payload.filename) {
        currentFile.value = null;
      }
      if (payload.phase === 'syncing') {
        status.value = 'syncing';
      } else if (payload.phase === 'completed') {
        status.value = 'completed';
        connected.value = true;
      }
    },
  );

  void listenEvent('sync-error', (payload) => {
    status.value = 'error';
    error.value = payload.message;
    connected.value = false;
  });

  void listenEvent('webrtc-state', (payload: string) => {
    const s = (payload ?? '').toLowerCase();
    if (s === 'connected' || s.includes('peer connected')) {
      connection.value = 'connected';
      connected.value = true;
    } else if (
      s.includes('disconnect') ||
      s.includes('failed') ||
      s.includes('closed') ||
      s.includes('error')
    ) {
      connection.value = 'offline';
      connected.value = false;
    }
  });

  void listenEvent('peer-connected', (payload: { device_id: string }) => {
    connection.value = 'connected';
    connected.value = true;
    if (payload?.device_id) {
      activePeerId.value = payload.device_id;
    }
  });

  void listenEvent('peer-disconnected', (payload: string) => {
    connection.value = 'offline';
    connected.value = false;
    viewOnlyActive.value = false;
    viewOnlyPhotos.value = [];
    if (typeof payload === 'string' && payload) {
      activePeerId.value = payload;
    }
  });

  void listenEvent('view-manifest', (payload: string) => {
    try {
      const photos = JSON.parse(payload) as ViewPhoto[];
      viewOnlyPhotos.value = Array.isArray(photos) ? photos : [];
      viewOnlyLoading.value = false;
    } catch (e) {
      console.error('[SyncStore] Failed to parse view manifest:', e);
      viewOnlyLoading.value = false;
    }
  });

  /** Ask the peer for a read-only look at their library (#9). */
  async function browseOnly(): Promise<void> {
    viewOnlyLoading.value = true;
    viewOnlyPhotos.value = [];
    viewOnlyActive.value = true;
    try {
      await enterViewOnly();
    } catch (e) {
      console.error('[SyncStore] enter_view_only failed:', e);
      viewOnlyActive.value = false;
      viewOnlyLoading.value = false;
    }
  }

  function exitViewOnly(): void {
    viewOnlyActive.value = false;
    viewOnlyPhotos.value = [];
    // Backend state is reset on disconnect/stop; the sharer keeps serving
    // nothing once we stop requesting.
  }

  async function reconnect(): Promise<boolean> {
    try {
      const ok = await autoReconnect();
      if (ok) {
        connection.value = 'connected';
        connected.value = true;
      }
      return ok;
    } catch (e) {
      console.error('[SyncStore] reconnect failed:', e);
      connection.value = 'offline';
      return false;
    }
  }

  return {
    status,
    progress,
    error,
    currentFile,
    devices,
    connected,
    connection,
    connectionMode,
    activePeerId,
    viewOnlyActive,
    viewOnlyPhotos,
    viewOnlyLoading,
    loadDevices,
    reconnect,
    browseOnly,
    exitViewOnly,
  };
});
