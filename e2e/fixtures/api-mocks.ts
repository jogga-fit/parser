import { test as base } from '@playwright/test';

// No custom fixture values — this fixture exists solely to block external
// font requests on every page so screenshots are deterministic.
export type ApiMocksFixtures = {
  _fontsBlocked: void;
};

export const test = base.extend<ApiMocksFixtures>({
  _fontsBlocked: [
    async ({ page }, use) => {
      await page.route('https://fonts.googleapis.com/**', (r) =>
        r.fulfill({ contentType: 'text/css', body: '' }),
      );
      await page.route('https://fonts.gstatic.com/**', (r) =>
        r.fulfill({ contentType: 'font/woff2', body: '' }),
      );
      await use();
    },
    { auto: true },
  ],
});

export { expect } from '@playwright/test';
