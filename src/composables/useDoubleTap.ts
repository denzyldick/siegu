import { ref } from 'vue';

const TAP_DELAY = 280;

export function useDoubleTap(onSingleTap: () => void, onDoubleTap: () => void) {
  let lastTap = 0;
  let openTimer: ReturnType<typeof setTimeout> | undefined;
  const heartPop = ref(false);
  let popTimer: ReturnType<typeof setTimeout> | undefined;

  function triggerHeartPop(): void {
    heartPop.value = false;
    void 0; // force flush
    heartPop.value = true;
    if (popTimer !== undefined) clearTimeout(popTimer);
    popTimer = setTimeout(() => {
      heartPop.value = false;
      popTimer = undefined;
    }, 800);
  }

  function handleTap(): void {
    const now = Date.now();
    if (now - lastTap < TAP_DELAY) {
      lastTap = 0;
      if (openTimer !== undefined) {
        clearTimeout(openTimer);
        openTimer = undefined;
      }
      onDoubleTap();
      triggerHeartPop();
      return;
    }
    lastTap = now;
    openTimer = setTimeout(() => {
      openTimer = undefined;
      onSingleTap();
    }, TAP_DELAY);
  }

  function cancelPending(): void {
    if (openTimer !== undefined) {
      clearTimeout(openTimer);
      openTimer = undefined;
    }
    lastTap = 0;
  }

  return { handleTap, heartPop, cancelPending };
}
