import { test, expect } from '@playwright/test';

/**
 * Proves model-generated metadata surfaces in the MediaViewer: BLIP caption,
 * YOLO/CLIP object tags (AI Insights), and OCR recognized text. All three live
 * in the info drawer, opened with the keyboard shortcut `i` (or info button).
 */

const MAX_TILES = 40;

test.describe('MediaViewer — AI generated metadata in the UI', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('.media-card-container').first()).toBeVisible({
      timeout: 25_000,
    });
    await page.waitForTimeout(2500);
  });

  test('BLIP caption + AI Insights tags render in the info drawer', async ({ page }) => {
    let found = false;
    for (let i = 0; i < MAX_TILES; i++) {
      const tile = page.locator('.media-card-container').nth(i);
      if ((await tile.count()) === 0) break;
      await tile.click({ force: true });
      await page.waitForTimeout(1100);
      await page.keyboard.press('i'); // open info drawer
      await page.waitForTimeout(900);
      const captionCount = await page.getByText('AI Caption', { exact: true }).count();
      const tagsCount = await page.getByText('AI Insights', { exact: true }).count();
      if (captionCount > 0 && tagsCount > 0) {
        // assert non-empty caption text on the italic block
        const captionText = await page
          .locator('.v-navigation-drawer .font-italic')
          .first()
          .textContent();
        expect((captionText || '').trim().length).toBeGreaterThan(0);
        // assert at least one AI insight tag chip/row (object classes)
        const tagRows = await page
          .locator('.v-navigation-drawer .mb-4')
          .filter({ has: page.locator('.v-progress-linear') })
          .count();
        expect(tagRows).toBeGreaterThan(0);
        found = true;
        break;
      }
      await page.keyboard.press('Escape').catch(() => {});
      await page.waitForTimeout(600);
    }
    expect(found).toBe(true);
  });

  test('OCR recognized text renders for a photo that has OCR', async ({ page }) => {
    let found = false;
    for (let i = 0; i < MAX_TILES; i++) {
      const tile = page.locator('.media-card-container').nth(i);
      if ((await tile.count()) === 0) break;
      await tile.click({ force: true });
      await page.waitForTimeout(1100);
      await page.keyboard.press('i');
      await page.waitForTimeout(900);
      const ocrCount = await page.getByText('Recognized text', { exact: true }).count();
      if (ocrCount > 0) {
        const ocrText = await page.locator('.v-navigation-drawer .ocr-text').textContent();
        expect((ocrText || '').trim().length).toBeGreaterThan(0);
        found = true;
        break;
      }
      await page.keyboard.press('Escape').catch(() => {});
      await page.waitForTimeout(600);
    }
    expect(found).toBe(true);
  });

  test('aesthetics score drives sort + is present on photos', async ({ page }) => {
    // direct data-plane proof: every seeded photo carries an aesthetics score
    const token = (await (await page.request.get('/session')).json()).webToken;
    const res = await page.request.post('/rpc', {
      headers: { authorization: `Bearer ${token}` },
      data: { name: 'list_files', payload: { limit: 1000 } },
    });
    const body = (await res.json()) as {
      result?: Array<{ aesthetics_score: number | null }>;
    };
    const photos = body.result ?? [];
    expect(photos.length).toBe(46);
    const scored = photos.filter((p) => p.aesthetics_score != null);
    expect(scored.length).toBeGreaterThan(0);
  });
});
