import { storeToRefs } from 'pinia';
import { useSettingsStore } from '@/stores/settings';

/**
 * Compatibility facade over the settings Pinia store.
 *
 * Returns the same shape the composable used to produce (state refs, computed
 * refs, reactive objects and actions) so callers can keep destructuring.
 * Reactive state like `performance` and the dialog/snackbar objects are kept as
 * the reactive objects themselves, while refs/computeds come back as refs.
 */
export function useSettings() {
  const store = useSettingsStore();
  return {
    ...store,
    ...storeToRefs(store),
    performance: store.performance,
    snackbar: store.snackbar,
    cleanupDialog: store.cleanupDialog,
    removeFolderDialog: store.removeFolderDialog,
  };
}
