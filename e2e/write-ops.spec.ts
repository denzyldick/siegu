import { test, expect } from '@playwright/test';

/**
 * Write-path proofs in webHost --share-mode rw mode: the exact RPCs the UI
 * toolbar/heart invokes (toggle_favorite) must persist a change that a
 * subsequent filtered read observes. Host is rw, so mutations are allowed.
 */

async function sessionToken(page: import('@playwright/test').Page): Promise<string> {
  return ((await (await page.request.get('/session')).json()) as { webToken: string }).webToken;
}

async function listFiles(
  page: import('@playwright/test').Page,
  payload: Record<string, unknown>,
): Promise<unknown[]> {
  const token = await sessionToken(page);
  const res = await page.request.post('/rpc', {
    headers: { authorization: `Bearer ${token}` },
    data: { name: 'list_files', payload },
  });
  const body = (await res.json()) as { result?: unknown[]; error?: string };
  expect(body.error, `list_files error: ${body.error}`).toBeUndefined();
  return body.result ?? [];
}

test.describe('Write-path (rw) — favorite toggle persists', () => {
  test('toggling favorite is persisted and observed on a filtered read', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('.media-card-container').first()).toBeVisible({ timeout: 25_000 });

    const token = await sessionToken(page);
    const all = (await listFiles(page, { limit: 1000 })) as Array<{ id: string; name?: string }>;
    expect(all.length).toBe(46);
    const target = all[0]!;

    // read initial favorite state of the target
    const favBefore = await listFiles(page, { limit: 1000, favorites_only: true });
    const initiallyFav =
      favBefore.filter((p) => (p as { id: string }).id === target.id).length === 1;

    // toggle via the UI's RPC
    const toggle = await page.request.post('/rpc', {
      headers: { authorization: `Bearer ${token}` },
      data: { name: 'toggle_favorite', payload: { id: target.id } },
    });
    const tBody = (await toggle.json()) as { ok?: boolean; result?: unknown; error?: string };
    expect(tBody.error, `toggle error: ${tBody.error}`).toBeUndefined();

    // observe via favorites_only filtered read
    const favAfter = await listFiles(page, { limit: 1000, favorites_only: true });
    const nowFav = favAfter.filter((p) => (p as { id: string }).id === target.id).length === 1;
    expect(nowFav).toBe(!initiallyFav);

    // revert so the demo dataset stays clean
    await page.request.post('/rpc', {
      headers: { authorization: `Bearer ${token}` },
      data: { name: 'toggle_favorite', payload: { id: target.id } },
    });
  });
});
