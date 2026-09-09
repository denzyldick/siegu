const { chromium } = require('playwright');
const { readdirSync } = require('fs');

const BASE = 'http://localhost:8788/';
const OUT = '/tmp/opencode/shots-v5';
const fs = require('fs');
fs.mkdirSync(OUT, { recursive: true });

async function shot(page, name) {
  await page.screenshot({ path: `${OUT}/${name}.png` });
  console.log('saved', name);
}
async function clickDock(page, icon) {
  await page.hover('.dock-container').catch(() => {});
  await page.waitForTimeout(400);
  const btn = page.locator(`i.mdi-${icon}, .mdi-${icon}`).last();
  await btn.scrollIntoViewIfNeeded().catch(() => {});
  await btn.click({ force: true });
  await page.waitForTimeout(1800);
  await page.mouse.move(1200, 400);
}

(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1920, height: 1200 },
    deviceScaleFactor: 1,
  });
  page.setDefaultTimeout(60000);
  await page.goto(BASE, { waitUntil: 'domcontentloaded', timeout: 90000 });
  await page.waitForSelector('img', { timeout: 45000 }).catch(() => {});
  await page.waitForTimeout(7000);
  console.log('img tiles:', await page.locator('img').count());

  // 1. Space Saver with a real scan result
  await clickDock(page, 'file-multiple-outline');
  await page.waitForTimeout(1500);
  const scanBtn = page.getByText(/scan|start/i).first();
  if (await scanBtn.isVisible().catch(() => false)) {
    await scanBtn.click({ force: true });
    console.log('clicked scan button');
  } else {
    console.log('no visible scan button');
  }
  await page.waitForTimeout(6500);
  await shot(page, '1-space-saver-result');

  // 2. Google-style search with tasteful query
  await page.hover('.dock-container').catch(() => {});
  await page.waitForTimeout(300);
  await page.locator('.siegu-logo-wrap').click({ force: true }).catch(() => {});
  await page.waitForTimeout(1800);
  await page.mouse.move(1200, 400);
  await page.waitForTimeout(1000);
  const input = page.locator('.search-input').first();
  if (await input.isVisible().catch(() => false)) {
    await input.fill('sunsets');
    await page.waitForTimeout(3500);
    await shot(page, '2-search');
    await input.fill('').catch(() => {});
  } else {
    console.log('no .search-input');
    await shot(page, '2-search-fallback');
  }

  // 3. Collections with covers
  await clickDock(page, 'album');
  await page.waitForTimeout(3000);
  await shot(page, '3-collections');

  // 4. Devices (sync screenshot)
  await clickDock(page, 'laptop');
  await page.waitForTimeout(2500);
  await shot(page, '4-devices');

  await browser.close();
  console.log('DONE');
})().catch((e) => {
  console.error('ERR', e.message);
  process.exit(1);
});