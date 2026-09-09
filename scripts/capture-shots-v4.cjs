const { chromium } = require('playwright');

const BASE = "http://localhost:8788/";
const OUT = '/tmp/opencode/shots-v3';
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
  await page.waitForTimeout(1600);
  await page.mouse.move(1200, 400);
}

(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1920, height: 1200 },
    deviceScaleFactor: 1,
  });
  page.setDefaultTimeout(90000);

  await page.goto(BASE, { waitUntil: 'domcontentloaded' });
  console.log('goto ok');

  // wait for app shell + grid tiles
  await page
    .waitForSelector('text=Siegu', { timeout: 45000 })
    .catch(() => console.log('no Siegu title'));
  await page.waitForSelector('img', { timeout: 45000 }).catch(() => console.log('no img'));
  await page.waitForTimeout(8000);
  const tiles = await page.locator('img').count();
  console.log('img tiles visible:', tiles);
  await shot(page, '1-library');

  // share dialog: click a share trigger in viewer later

  // 2. Open first tile -> media viewer
  await page
    .locator('img')
    .nth(3)
    .click({ force: true })
    .catch(() => console.log('tile click failed'));
  await page.waitForTimeout(3500);
  await shot(page, '2-viewer-photo');

  // close viewer
  await page.keyboard.press('Escape');
  await page.waitForTimeout(1200);

  // 3. Space Saver
  await clickDock(page, 'file-multiple-outline');
  await page.waitForTimeout(2500);
  await shot(page, '3-space-saver-idle');

  // start scan if a button exists
  const scanbtn = page.getByText(/Scan for duplicates|Start scan/i).first();
  if (await scanbtn.isVisible().catch(() => false)) {
    await scanbtn.click();
    await page.waitForTimeout(9000);
  }
  await page.waitForTimeout(3000);
  await shot(page, '3b-space-saver-result');

  // 4. Collections
  await clickDock(page, 'album');
  await page.waitForTimeout(2500);
  await shot(page, '4-collections');

  // 5. Map
  await clickDock(page, 'map-outline');
  await page.waitForTimeout(3000);
  await shot(page, '5-map');

  // 6. Devices
  await clickDock(page, 'laptop');
  await page.waitForTimeout(2000);
  await shot(page, '6-devices');

  // 7. Settings
  await clickDock(page, 'cog-outline');
  await page.waitForTimeout(2000);
  await shot(page, '7-settings');

  // 8. Search
  await page.hover('.dock-container').catch(() => {});
  await page.waitForTimeout(300);
  await page.locator('.siegu-logo-wrap').click({ force: true });
  await page.waitForTimeout(1800);
  await page.mouse.move(1200, 400);
  await page.waitForTimeout(1800);
  const searchInput = page.locator('.search-input').first();
  if (await searchInput.isVisible().catch(() => false)) {
    await searchInput.fill('sunsets');
    await page.waitForTimeout(3500);
    await shot(page, '8-search');
    await searchInput.fill('').catch(() => {});
  } else {
    console.log('no search input found');
  }

  await browser.close();
  console.log('DONE');
})().catch((e) => {
  console.error('ERR', e);
  process.exit(1);
});
