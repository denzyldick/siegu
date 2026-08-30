import { defineConfig, devices } from '@playwright/test';

// Default: run against an externally-started stack (host on SIEGU_WEB_HOST,
// vite proxying it on 1420). Set E2E_SELF_START=1 to have Playwright boot vite
// itself (host must still be running; see docs/e2e.md).
const selfStart = process.env.E2E_SELF_START === '1';
const webPort = Number(process.env.E2E_WEB_PORT || 1420);

export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  expect: { timeout: 12_000 },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report' }]],
  use: {
    baseURL: `http://localhost:${webPort}`,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'off',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: selfStart
    ? {
        command: `SIEGU_WEB_HOST=${process.env.SIEGU_WEB_HOST || 'http://localhost:8788'} npm run dev -- --port ${webPort} --strictPort`,
        port: webPort,
        reuseExistingServer: true,
        timeout: 60_000,
      }
    : undefined,
});
