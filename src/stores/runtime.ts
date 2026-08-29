/**
 * Runtime-mode store (#24, Phase 1).
 *
 * Exposes the detected UI mode (`tauri` / `webHost` / `guest` / `onboarding`)
 * and a lazily-created shared `Backend` instance so stores and components drive
 * data/media through one seam regardless of mode.
 *
 * `initRuntime()` MUST be awaited at boot (e.g. in `App.vue` onMounted) before
 * mode-sensitive logic runs; mode is `null` until then.
 */
import { defineStore } from 'pinia';
import { shallowRef, ref, computed } from 'vue';
import { detectMode, type DetectedMode } from '@/services/runtime';
import { bootGuest, type GuestBootEvents } from '@/services/backend/bootGuest';
import { createBackend } from '@/services/backend/createBackend';
import type { GuestClient } from '@/services/backend/guest';
import type { PeerTransport } from '@/services/backend/peer';
import type { Backend, RuntimeMode } from '@/services/backend/interface';

export type GuestConnectionState = 'idle' | 'connecting' | 'connected' | 'closed' | 'error';

export const useRuntimeStore = defineStore('runtime', () => {
  const mode = shallowRef<RuntimeMode | null>(null);
  /** The detected guest session (`#code.token`), present when mode is guest. */
  const session = shallowRef<DetectedMode['session']>(undefined);
  /** The host-issued webHost data-plane token, present when mode is webHost. */
  const webHostToken = shallowRef<DetectedMode['webHostToken']>(undefined);

  /** For guest Mode B: the paired `GuestClient`, once connected. */
  const guestClient = shallowRef<GuestClient | null>(null);
  const guestConnection = ref<GuestConnectionState>('idle');
  const guestError = ref('');

  const isDesktop = computed(() => mode.value === 'tauri');
  const isWebHost = computed(() => mode.value === 'webHost');
  const isGuest = computed(() => mode.value === 'guest');
  const isOnboarding = computed(() => mode.value === 'onboarding');

  const isGuestConnected = computed(
    () => mode.value === 'guest' && guestConnection.value === 'connected',
  );

  const backend = computed<Backend>(() => {
    if (mode.value === 'guest') {
      const client = guestClient.value;
      if (client) return createBackend('guest', client);
    }
    if (mode.value === 'webHost') {
      return createBackend('webHost', undefined, webHostToken.value);
    }
    return createBackend('tauri');
  });

  /** Resolve the runtime mode once at boot. Safe to call repeatedly. */
  async function initRuntime(): Promise<void> {
    if (mode.value !== null) return;
    const detected = await detectMode();
    mode.value = detected.mode;
    session.value = detected.session;
    webHostToken.value = detected.webHostToken;
  }

  /**
   * Initiate guest Mode B pairing using the detected session. Used from App boot
   * and from the connect screen when a code is entered manually.
   */
  async function connectGuest(
    s: DetectedMode['session'],
    events: GuestBootEvents = {},
    transportOverride?: PeerTransport,
  ): Promise<GuestClient> {
    if (!s) throw new Error('No guest session to connect with');
    guestConnection.value = 'connecting';
    guestError.value = '';

    const { client } = bootGuest(
      s,
      {
        onOpen: () => {
          guestConnection.value = 'connected';
          events.onOpen?.();
        },
        onClose: () => {
          if (guestConnection.value === 'connected') guestConnection.value = 'closed';
          events.onClose?.();
        },
        onError: (m) => {
          guestConnection.value = 'error';
          guestError.value = m;
          events.onError?.(m);
        },
        onMedia: (id, key, url) => events.onMedia?.(id, key, url),
      },
      transportOverride,
    );

    guestClient.value = client;
    return client;
  }

  /**
   * If this boot resolves to guest mode, connect automatically (used by App.vue).
   */
  async function maybeConnectGuest(autoEvents: GuestBootEvents = {}): Promise<void> {
    const s = session.value;
    if (mode.value !== 'guest' || !s) return;
    try {
      await connectGuest(s, autoEvents);
    } catch (e) {
      guestConnection.value = 'error';
      guestError.value = e instanceof Error ? e.message : String(e);
    }
  }

  function setGuestClient(client: GuestClient | null): void {
    guestClient.value = client;
  }

  return {
    mode,
    session,
    webHostToken,
    guestClient,
    guestConnection,
    guestError,
    isDesktop,
    isWebHost,
    isGuest,
    isGuestConnected,
    isOnboarding,
    backend,
    initRuntime,
    connectGuest,
    maybeConnectGuest,
    setGuestClient,
  };
});
