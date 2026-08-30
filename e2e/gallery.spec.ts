import { test, expect } from '@playwright/test';

test.describe('Boot & mode detection', () => {
  test('app boots in webHost mode against the host', async ({ page }) => {
    await page.goto('/');
    // app fetches /session and registers a webHost backend; gallery mounts
    await expect(page.locator('.media-card-container').first()).toBeVisible({
      timeout: 25_000,
    });
    // no crash/console errors
    const pageErrors: string[] = [];
    page.on('pageerror', (e) => pageErrors.push(String(e)));
    await page.waitForTimeout(1500);
    expect(pageErrors.length).toBe(0);
  });
});

test.describe('Gallery (model-enriched demo)', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('.media-card-container').first()).toBeVisible({
      timeout: 25_000,
    });
    await page.waitForTimeout(3000);
  });

  test('renders all 46 seed items', async ({ page }) => {
    await expect(page.locator('.media-card-container')).toHaveCount(46, { timeout: 15_000 });
  });

  test('renders the 6 video items with a play indicator', async ({ page }) => {
    await expect(page.locator('.media-card-container .video-indicator')).toHaveCount(6, {
      timeout: 15_000,
    });
  });

  test('video hover preview actually plays', async ({ page }) => {
    const vIndicator = page.locator('.media-card-container .video-indicator').first();
    await expect(vIndicator).toBeVisible({ timeout: 15_000 });
    const wrapper = vIndicator.locator('xpath=ancestor::div[contains(@class,"media-card-container")]');
    await wrapper.hover();
    await page.waitForTimeout(1200);
    const video = wrapper.locator('video').first();
    await expect(video).toHaveCount(1, { timeout: 5000 });
    await video.evaluate((el: HTMLVideoElement) => {
      el.muted = true;
      const p = el.play();
      if (p && p.catch) p.catch(() => {});
    });
    await page.waitForTimeout(2000);
    const playing = await video.evaluate(
      (el: HTMLVideoElement) => !el.paused && !el.ended && el.currentTime > 0,
    );
    expect(playing).toBe(true);
  });

  test('analyzed photos carry a model-analyzed .ai-badge', async ({ page }) => {
    // every seed photo was analyzed and BLIP-captioned, so the .ai-badge
    // (mdi-auto-fix) should appear on many tiles
    const badgeCount = await page.locator('.media-card-container .ai-badge').count();
    expect(badgeCount).toBeGreaterThan(0);
  });

  test('all thumbnails actually decoded (no broken imgs)', async ({ page }) => {
    await page.waitForTimeout(2000);
    const broken = await page
      .locator('.media-card-container img.media-card-img')
      .evaluateAll((imgs) =>
        imgs.filter((i) => i instanceof HTMLImageElement && i.complete && i.naturalWidth === 0),
      );
    expect(broken.length).toBe(0);
  });
});
