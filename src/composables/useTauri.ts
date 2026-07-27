import { onUnmounted } from 'vue'
import { listenEvent as subscribeEvent } from '@/services/events'
import type { TauriEventMap, TauriEventName } from '@/types/events'

export function useTauri() {
  const cleanupFns: Array<() => void> = []
  let unmounted = false

  function listen<K extends TauriEventName>(
    eventName: K,
    handler: (payload: TauriEventMap[K]['payload']) => void,
  ): void {
    void subscribeEvent<K>(eventName, handler).then((unlisten) => {
      if (unmounted) {
        unlisten()
      } else {
        cleanupFns.push(unlisten)
      }
    })
  }

  onUnmounted(() => {
    unmounted = true
    for (const cleanup of cleanupFns) {
      cleanup()
    }
    cleanupFns.length = 0
  })

  return { listen }
}
