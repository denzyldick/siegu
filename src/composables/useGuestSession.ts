/**
 * Guest session lifecycle for the shared-collection experience.
 *
 * Drives the connecting/connected/error states and enforces the timed
 * (`minutes`) and one-time (close on session end) limits carried by the
 * share link's URL fragment (`#CODE.TOKEN.ALBUM[.MIN|.once]`).
 */
import { ref, watch, onUnmounted, computed, type Ref } from 'vue';
import { useRuntimeStore, type GuestConnectionState } from '@/stores/runtime';

export interface GuestSessionState {
  /** True once the guest accepted and the intro overlay is dismissed. */
  revealed: Ref<boolean>;
  /** True once the shared session has ended (expiry or host close). */
  ended: Ref<boolean>;
  /** Human-facing connection status for the guest header/overlay. */
  status: Ref<'idle' | 'connecting' | 'connected' | 'closed' | 'error'>;
  reveal(): void;
}

export function useGuestSession(): GuestSessionState {
  const runtime = useRuntimeStore();
  const revealed = ref(false);
  const ended = ref(false);
  const status = ref<'idle' | 'connecting' | 'connected' | 'closed' | 'error'>('idle');

  let expiryTimer: ReturnType<typeof setTimeout> | undefined;
  let wasConnected = false;

  const runtimeStatus = computed<GuestConnectionState>(() => runtime.guestConnection);
  watch(runtimeStatus, (s) => {
    if (s === 'connecting') {
      status.value = 'connecting';
    } else if (s === 'connected') {
      status.value = 'connected';
      if (!wasConnected) {
        wasConnected = true;
        scheduleExpiry();
      }
    } else if (s === 'closed') {
      status.value = 'closed';
      if (wasConnected) markEnded();
    } else if (s === 'error') {
      status.value = 'error';
    }
  });

  function scheduleExpiry(): void {
    const minutes = runtime.session?.minutes;
    if (!minutes || minutes <= 0) return;
    clearTimeout(expiryTimer);
    expiryTimer = setTimeout(() => markEnded(), minutes * 60 * 1000);
  }

  function markEnded(): void {
    if (ended.value) return;
    ended.value = true;
    status.value = 'closed';
    revealed.value = false;
  }

  function reveal(): void {
    revealed.value = true;
  }

  onUnmounted(() => clearTimeout(expiryTimer));

  return { revealed, ended, status, reveal };
}
