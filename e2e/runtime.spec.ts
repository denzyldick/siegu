import { test, expect } from '@playwright/test';

/**
 * Boundary guarantee: this browser E2E exercises the SAME shared Vue UI that
 * runs on desktop/Android under the Tauri backend. This spec proves the test
 * actually runs in plain-browser webHost mode (no Tauri IPC), so that when
 * these specs pass, the shared UI is validated — leaving only the thin Tauri
 * native window (IPC, file dialogs, sync/mesh, wallpaper) as platform-specific.
 */

test.describe('Runtime boundary — plain-browser webHost mode', () => {
  test('no Tauri internals are present (plain browser, not the Tauri webview)', async ({
    page,
  }) => {
    await page.goto('/');
    const hasTauri = await page.evaluate(
      () =>
        Boolean(
          // @ts-expect-error Tauri injects internals on window
          (window as Record<string, unknown>)['__TAURI_INTERNALS__'] ||
            // @ts-expect-error older IPC
            (window as Record<string, unknown>)['__TAURI__'],
        ),
    );
    expect(hasTauri).toBe(false);
  });

  test('the webHost data plane (session + rpc) is the active backend', async ({ page }) => {
    const sessRes = await page.request.get('/session');
    expect(sessRes.ok()).toBe(true);
    const sess = (await sessRes.json()) as { webToken: string; code: string };
    expect(sess).toHaveProperty('webToken');
    expect(sess).toHaveProperty('code');

    const rpcRes = await page.request.post('/rpc', {
      headers: { authorization: `Bearer ${sess.webToken}` },
      data: { name: 'list_files', payload: { limit: 1000 } },
    });
    expect(rpcRes.ok()).toBe(true);
    const body = (await rpcRes.json()) as { result?: unknown[]; error?: string };
    expect(body.error).toBeUndefined();
    expect((body.result ?? []).length).toBe(46);
  });

  test('gallery boots to the webHost-served model-enriched library', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('.media-card-container').first()).toBeVisible({
      timeout: 25_000,
    });
    await expect(page.locator('.media-card-container')).toHaveCount(46, { timeout: 15_000 });
  });
});
