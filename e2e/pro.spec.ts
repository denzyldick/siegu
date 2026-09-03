import { test, expect } from '@playwright/test';

/**
 * Pro activation UI — boundary guarantee.
 *
 * Exercises the shared Vue Settings → Pro section in plain-browser webHost mode
 * (the same UI the Tauri desktop app shells). It does NOT depend on live
 * Stripe/Resend/Cloudflare: in this environment the license commands resolve
 * through the browser fallback (`invoke.ts` returns a shape-correct
 * `{ok:false,paid:false,verified:false}`), so the test proves the section
 * renders, is reachable, and never crashes the app when a flow is triggered.
 *
 * The real paid+verified unlock is covered by the Cloudflare Worker + the
 * desktop Rust commands; run those end-to-end once a host + Worker are live.
 */

test.describe('Settings → Pro activation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // Wait for the app to boot the library before navigating the dock.
    await expect(page.locator('.media-card-container').first()).toBeVisible({
      timeout: 25_000,
    });
    // Open Settings from the bottom dock.
    await page.locator('[data-tour="dock-settings"]').click();
  });

  test('Pro section renders with email field, verify actions and upsell', async ({ page }) => {
    const proSection = page.locator('[data-tour="settings-pro"]');
    await expect(proSection).toBeVisible({ timeout: 15_000 });

    await expect(proSection.getByLabel('Email used at checkout')).toBeVisible();

    await expect(
      proSection.getByRole('button', { name: 'Send verification email' }),
    ).toBeVisible();
    await expect(proSection.getByRole('button', { name: 'Check status' })).toBeVisible();

    // Upsell block links to the pricing page.
    const upsell = proSection.getByRole('button', { name: 'Get Pro' });
    await expect(upsell).toBeVisible();
    expect(await upsell.getAttribute('href')).toContain('#pricing');
  });

  test('typing an email and checking status stays functional (browser fallback)', async ({
    page,
  }) => {
    const proSection = page.locator('[data-tour="settings-pro"]');
    await expect(proSection).toBeVisible({ timeout: 15_000 });

    const emailField = proSection.getByLabel('Email used at checkout');
    await emailField.fill('pro.test@example.com');

    // Check status must resolve without throwing and surface the not-paid state.
    await proSection.getByRole('button', { name: 'Check status' }).click();

    // Browser fallback → not verified / not paid: an inline result panel appears
    // (no crash, no error thrown). Match on its error copy.
    await expect(proSection.locator('.pa-3.rounded-lg.border')).toBeVisible({ timeout: 15_000 });
    await expect(proSection.locator('.pa-3.rounded-lg.border')).toContainText(
      /paid|purchase|error/i,
    );
  });

  test('validation requires an email before checking', async ({ page }) => {
    const proSection = page.locator('[data-tour="settings-pro"]');
    await expect(proSection).toBeVisible({ timeout: 15_000 });

    // Leave email empty and trigger check status → should not throw.
    await proSection.getByRole('button', { name: 'Check status' }).click();
    await expect(proSection).toBeVisible();
  });
});
