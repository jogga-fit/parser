/**
 * Navigation tests for Jogga — single-user Dioxus fullstack app.
 *
 * Tests marked with SKIP_INTEGRATION require a running server with a seeded
 * owner account (INTEGRATION_TEST=true). Others only need the app to serve HTML.
 *
 * Run integration tests:
 *   INTEGRATION_TEST=true npm test -- --grep "navigation"
 */
import { test, expect } from '@playwright/test';
import { waitForHydration } from '../fixtures/helpers';
import { WEB, OWNER_PASSWORD, SKIP_INTEGRATION } from '../fixtures/env';

const OWNER_USERNAME = process.env.OWNER_USERNAME || 'owner';

// ---------------------------------------------------------------------------
// Owner profile routing
// ---------------------------------------------------------------------------

test.describe('Navigation — owner profile routing', () => {
  test('/ shows owner profile for unauthenticated visitor', async ({ page }) => {
    test.skip(SKIP_INTEGRATION, 'set INTEGRATION_TEST=true for this test');

    await page.goto(`${WEB}/`);
    await waitForHydration(page);

    // The root redirects to or renders the owner profile
    const profileCard = page.locator('.profile-card');
    const profileHandle = page.locator('.profile-handle');
    await expect(profileCard.or(profileHandle).first()).toBeVisible();
  });

  test(`/@{owner} shows owner profile`, async ({ page }) => {
    test.skip(SKIP_INTEGRATION, 'set INTEGRATION_TEST=true for this test');

    await page.goto(`${WEB}/@${OWNER_USERNAME}`);
    await waitForHydration(page);

    await expect(page.locator('.profile-card')).toBeVisible();
  });

  test('/@nonexistent shows single-user not-found card', async ({ page }) => {
    test.skip(SKIP_INTEGRATION, 'set INTEGRATION_TEST=true for this test');

    await page.goto(`${WEB}/@nobody_here_xyz`);
    await waitForHydration(page);

    // Profile-level not-found renders .not-found-card with text about single-user
    await expect(page.locator('.not-found-card')).toBeVisible();
    await expect(page.locator('.nf-desc')).toContainText('single-user');
  });
});

// ---------------------------------------------------------------------------
// Auth pages (no integration needed)
// ---------------------------------------------------------------------------

test.describe('Navigation — auth pages', () => {
  test('/login renders sign-in form', async ({ page }) => {
    await page.goto(`${WEB}/login`);
    await waitForHydration(page);

    await expect(page.locator('.auth-page')).toBeVisible();
    await expect(page.locator('#login-field')).toBeVisible();
    await expect(page.locator('input[autocomplete="current-password"]')).toBeVisible();
    await expect(page.locator('button:has-text("Sign in")')).toBeVisible();
  });

  test('/reset-password without code shows password-set form', async ({ page }) => {
    await page.goto(`${WEB}/reset-password`);
    await waitForHydration(page);

    // Page loads the auth-page wrapper; without a valid code, the OTP form is shown
    await expect(page.locator('.auth-page')).toBeVisible();
    // Should still offer a way back to sign in
    await expect(page.locator('a:has-text("Back to sign in")')).toBeVisible();
  });

  test('/reset-password with expired code shows re-request hint after submit', async ({ page }) => {
    test.skip(SKIP_INTEGRATION, 'set INTEGRATION_TEST=true for this test');

    // Navigate with a clearly invalid OTP id; the form loads
    await page.goto(`${WEB}/reset-password?code=invalid-otp-id`);
    await waitForHydration(page);

    await expect(page.locator('.auth-page')).toBeVisible();

    // Fill in a dummy 6-char code and a password, then submit
    const otpInput = page.locator('input[name="otp"], input[placeholder*="code"], input[placeholder*="Code"]').first();
    const pwdInput = page.locator('input[autocomplete="new-password"]').first();
    const submitBtn = page.locator('button:has-text("Continue")');

    if (await otpInput.isVisible()) {
      await otpInput.fill('000000');
    }
    if (await pwdInput.isVisible()) {
      await pwdInput.fill('SomePass1!');
      // Fill confirm password if present
      const pwd2 = page.locator('input[autocomplete="new-password"]').nth(1);
      if (await pwd2.isVisible()) {
        await pwd2.fill('SomePass1!');
      }
    }
    if (await submitBtn.isVisible()) {
      await submitBtn.click();
    }

    // After submitting an invalid OTP the page transitions to "Code expired" state
    await expect(page.locator('.auth-hint')).toBeVisible({ timeout: 15_000 });
    await expect(page.locator('.auth-hint')).toContainText(/expired|already been used/);
  });
});

// ---------------------------------------------------------------------------
// DNQ catch-all 404
// ---------------------------------------------------------------------------

test.describe('Navigation — DNQ catch-all', () => {
  test('unknown path shows DNQ page', async ({ page }) => {
    await page.goto(`${WEB}/some/unknown/path/xyz`);
    await waitForHydration(page);

    await expect(page.locator('.not-found-page')).toBeVisible();
    await expect(page.locator('.nf-title')).toContainText('DNQ');
  });
});
