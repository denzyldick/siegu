import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@/services/invoke', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('@/services/signalling', () => ({
  DEFAULT_SIGNALING_URL: 'wss://example.test/ws',
  appendToken: (url: string, token: string) => `${url}?t=${token}`,
  getConfiguredSignalingUrl: vi.fn(async () => 'wss://example.test/ws'),
  pingSignalling: vi.fn(async () => ({ ok: true, latencyMs: 1, url: 'wss://example.test/ws' })),
  resolveSignalingBase: () => 'example.test',
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-os', () => ({ platform: vi.fn(() => 'linux') }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }));

import { useSettingsStore } from '@/stores/settings';

describe('settings store — Pro verify flow', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
  });

  it('sendProVerification persists paid_email only for a paid sender', async () => {
    const store = useSettingsStore();
    store.proEmail = 'buyer@example.com';
    invoke.mockResolvedValueOnce('{"ok":true,"sent":true,"email":"buyer@example.com"}');
    const status = await store.sendProVerification();
    expect(status.sent).toBe(true);
    const saved = invoke.mock.calls.filter(([name]) => name === 'save_config');
    expect(saved.map(([, args]) => args.key)).toContain('paid_email');
  });

  it('closes the verify dialog on timeout instead of leaving it stuck open', async () => {
    vi.useFakeTimers();
    try {
      const store = useSettingsStore();
      store.proEmail = 'buyer@example.com';
      // send_pro_verification succeeds; /verify never reports verified, so the
      // poll must eventually time out and close the dialog.
      invoke.mockImplementation((name: string) => {
        if (name === 'send_pro_verification') {
          return Promise.resolve('{"ok":true,"sent":true,"email":"buyer@example.com"}');
        }
        // verify_pro_email: paid but never verified
        return Promise.resolve('{"ok":true,"paid":true,"verified":false,"plan":"free"}');
      });

      const p = store.startProVerification();
      await vi.advanceTimersByTimeAsync(0);

      // Now step the poll interval repeatedly until past the 3-minute timeout.
      const started = Date.now();
      for (let i = 0; i < 4 && started; i++) {
        await vi.advanceTimersByTimeAsync(60_000);
      }

      await p;
      expect(store.proDialogOpen).toBe(false);
      expect(store.proVerifying).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });
});
