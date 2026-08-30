import { listen } from '@/services/invoke';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { TauriEventMap, TauriEventName } from '@/types/events';

export type { UnlistenFn };

export async function listenEvent<K extends TauriEventName>(
  eventName: K,
  handler: (payload: TauriEventMap[K]['payload']) => void,
): Promise<UnlistenFn> {
  return listen<TauriEventMap[K]['payload']>(eventName, (event) => {
    handler(event.payload);
  });
}
