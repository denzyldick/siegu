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
import { shallowRef, computed } from 'vue';
import { detectMode, type DetectedMode } from '@/services/runtime';
import { createBackend } from '@/services/backend/createBackend';
import type { GuestClient } from '@/services/backend/guest';
import type { Backend, RuntimeMode } from '@/services/backend/interface';

export const useRuntimeStore = defineStore('runtime', () => {
  const mode = shallowRef<RuntimeMode | null>(null);
  /** The detected guest session (`#code.token`), present when mode is guest. */
  const session = shallowRef<DetectedMode['session']>(undefined);

  /** For guest Mode B: the paired `GuestClient`, once connected. */
  const guestClient = shallowRef<GuestClient | null>(null);

  const isDesktop = computed(() => mode.value === 'tauri');
  const isWebHost = computed(() => mode.value === 'webHost');
  const isGuest = computed(() => mode.value === 'guest');
  const isOnboarding = computed(() => mode.value === 'onboarding');

  const backend = computed<Backend>(() => {
    if (mode.value === 'guest') {
      const client = guestClient.value;
      if (client) return createBackend('guest', client);
    }
    if (mode.value === 'webHost') {
      return createBackend('webHost');
    }
    return createBackend('tauri');
  });

  /** Resolve the runtime mode once at boot. Safe to call repeatedly. */
  async function initRuntime(): Promise<void> {
    if (mode.value !== null) return;
    const detected = await detectMode();
    mode.value = detected.mode;
    session.value = detected.session;
  }

  /** Set the guest client once Mode B pairing succeeds (see PHASE-2). */
  function setGuestClient(client: GuestClient | null): void {
    guestClient.value = client;
  }

  return {
    mode,
    session,
    isDesktop,
    isWebHost,
    isGuest,
    isOnboarding,
    backend,
    initRuntime,
    setGuestClient,
  };
});
