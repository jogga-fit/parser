import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './specs',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  // Single-user server: no parallel workers to avoid race conditions.
  workers: 1,
  timeout: 90_000,
  reporter: [['html', { open: 'never' }], ['line']],

  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:6060',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'on-first-retry',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
