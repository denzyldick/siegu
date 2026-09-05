import { reactive } from 'vue';

// Module-scoped singleton so the Pinia store (outside the component tree) can
// surface global notifications; App.vue renders the snackbar.
const state = reactive<{ show: boolean; text: string; color: string | undefined }>({
  show: false,
  text: '',
  color: undefined,
});

export function useGlobalSnackbar() {
  function show(text: string, color?: 'primary' | 'error' | 'success'): void {
    state.text = text;
    state.color = color;
    state.show = false;
    requestAnimationFrame(() => {
      state.show = true;
    });
  }

  return { state, show };
}