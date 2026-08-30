import type { Page } from '@playwright/test';

/**
 * E2E helpers running against the Vue app served by vite (which proxies
 * /session, /rpc, /thumb, /media to the `siegu web` host). The app boots in
 * webHost mode and fetches a fresh webToken from /session; our RPC helper does
 * the same so assertions can talk to the host data plane directly.
 * NOTE: import `test`/`expect` from '@playwright/test' in specs, not from here.
 */

export interface RpcResponse<T = unknown> {
  ok: boolean;
  result?: T;
  error?: string;
}

let cachedToken: string | null = null;

/** Fetch a webToken from the running host (via the vite proxy). */
export async function getWebToken(page: Page): Promise<string> {
  if (cachedToken) return cachedToken;
  const sess = await page.request.get('/session');
  const json = (await sess.json()) as { code: string; webToken: string };
  cachedToken = json.webToken;
  return json.webToken;
}

/** POST an RPC command to the host exactly as the webHost backend does. */
export async function rpc<T = unknown>(
  page: Page,
  name: string,
  payload: Record<string, unknown> = {},
): Promise<RpcResponse<T>> {
  const token = await getWebToken(page);
  const res = await page.request.post('/rpc', {
    headers: { authorization: `Bearer ${token}` },
    data: { name, payload },
  });
  return (await res.json()) as RpcResponse<T>;
}

/** Count media tiles currently in the gallery DOM. */
export async function countTiles(page: Page): Promise<number> {
  return page.locator('.media-card-container').count();
}

/** Open the MediaViewer on the nth gallery tile and open the info drawer. */
export async function openViewerInfo(page: Page, index = 0): Promise<void> {
  await page.locator('.media-card-container').nth(index).click({ force: true });
  await page.waitForTimeout(1200);
  await page.keyboard.press('i'); // toggles the info drawer (metadata + AI panel)
  await page.waitForTimeout(1000);
}

export const SELECTORS = {
  tile: '.media-card-container',
  tileBadge: '.media-card-container .analysed-badge, .media-card-container [class*="badge"]',
  nsfwBadge: '.media-card-container [class*="nsfw"]',
  videoIndicator: '.media-card-container .video-indicator',
  collectionsBtn: 'button[data-tour="dock-collections"]',
  collectionTile: '.collection-tile',
  searchInput: '.search-input',
  ocrText: '.ocr-text',
} as const;
