/**
 * Mobile UX tests — key user-facing flows at mobile viewport (390×844, iPhone 14).
 *
 * Covers mobile-specific layout behaviors (bottom nav, touch targets),
 * verifies core flows work on mobile, and captures reference screenshots to
 * e2e/screenshots/mobile-ux/ for QA review.
 *
 * All tests require INTEGRATION_TEST=true and a running server with a seeded
 * owner account (see e2e/README.md).
 *
 * Jogga is a single-owner server — all authenticated tests run as the owner.
 * Set OWNER_EMAIL and OWNER_PASSWORD in env or let them default to
 * owner@jogga.test / testpass99.
 *
 *   INTEGRATION_TEST=true npx playwright test mobile-ux --project=chromium
 */

import * as fs from 'fs';
import * as path from 'path';
import { test, expect } from '../fixtures/api-mocks';
import { createUser, injectAuth, uniqueSuffix } from '../fixtures/helpers';

const SKIP = !process.env.INTEGRATION_TEST;
const WEB = process.env.BASE_URL || 'http://localhost:6060';
const FIXTURE_GPX = path.join(__dirname, '../fixtures/test-activity.gpx');
const SS_DIR = path.join(__dirname, '..', 'screenshots', 'mobile-ux');

/** iPhone 14 — triggers CSS breakpoint that switches sidebar → bottom-nav. */
const MOBILE = { width: 390, height: 844 };

test.skip(SKIP, 'set INTEGRATION_TEST=true to run mobile-ux spec');

test.setTimeout(90_000);

test.beforeAll(() => {
  fs.mkdirSync(SS_DIR, { recursive: true });
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function waitForHydration(page: import('@playwright/test').Page) {
  await page.locator('body[data-hydrated]').waitFor({ state: 'attached', timeout: 60_000 });
}

async function uploadExercise(
  request: import('@playwright/test').APIRequestContext,
  token: string,
  title: string,
): Promise<void> {
  const gpxBuffer = fs.readFileSync(FIXTURE_GPX);
  const res = await request.post(`${WEB}/api/exercises/upload`, {
    headers: { Authorization: `Bearer ${token}` },
    multipart: {
      activityType: 'run',
      visibility: 'public',
      title,
      file: { name: 'activity.gpx', mimeType: 'application/gpx+xml', buffer: gpxBuffer },
    },
  });
  expect(res.status(), `upload "${title}"`).toBe(201);
}

function ss(name: string): string {
  return path.join(SS_DIR, name);
}

// ---------------------------------------------------------------------------
// Mobile nav — unauthenticated
// ---------------------------------------------------------------------------

test.describe('Mobile nav — unauthenticated', () => {
  test.use({ viewport: MOBILE });

  test('bottom nav is visible and sidebar is hidden', async ({ page }) => {
    await page.context().clearCookies();
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    await expect(page.locator('.bottom-nav')).toBeVisible();
    const sidebarDisplay = await page
      .locator('.sidebar')
      .evaluate((el) => window.getComputedStyle(el).display);
    expect(sidebarDisplay).toBe('none');
  });

  test('guest bottom nav shows Feed, People, Clubs; Sign in in mobile header', async ({ page }) => {
    await page.context().clearCookies();
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    await expect(page.locator('.bottom-nav-item', { hasText: 'Feed' })).toBeVisible();
    await expect(page.locator('.bottom-nav-item', { hasText: 'People' })).toBeVisible();
    await expect(page.locator('.bottom-nav-item', { hasText: 'Clubs' })).toBeVisible();
    // Sign in in mobile header (not bottom nav)
    await expect(page.locator('.mobile-header-signin')).toBeVisible();
    await expect(page.locator('.bottom-nav-item', { hasText: 'Sign in' })).not.toBeVisible();
    // Auth-only items absent
    await expect(page.locator('.sign-out-btn')).not.toBeVisible();
    await expect(page.locator('.bottom-nav-item', { hasText: 'Settings' })).not.toBeVisible();
    await expect(page.locator('.bottom-nav-item', { hasText: 'Notifications' })).not.toBeVisible();

    await page.screenshot({ path: ss('01-guest-bottom-nav.png'), fullPage: false });
  });

  test('mobile header Sign in link navigates to /login', async ({ page }) => {
    await page.context().clearCookies();
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    await page.locator('.mobile-header-signin').click();
    await expect(page).toHaveURL(new RegExp('/login'));
  });

  test('no horizontal overflow on /home', async ({ page }) => {
    await page.context().clearCookies();
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Mobile nav — authenticated
// ---------------------------------------------------------------------------

test.describe('Mobile nav — authenticated', () => {
  test.use({ viewport: MOBILE });

  test.beforeEach(async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);
  });

  test('authenticated bottom nav shows Feed/People/Clubs; header has bell and avatar', async ({ page }) => {
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);
    await page.locator('.page-content').waitFor({ state: 'visible', timeout: 10_000 });

    await expect(page.locator('.bottom-nav')).toBeVisible();
    await expect(page.locator('.bottom-nav-item', { hasText: 'Feed' })).toBeVisible();
    await expect(page.locator('.bottom-nav-item', { hasText: 'People' })).toBeVisible();
    await expect(page.locator('.bottom-nav-item', { hasText: 'Clubs' })).toBeVisible();
    // Sign in absent when authenticated
    await expect(page.locator('.mobile-header-signin')).not.toBeVisible();
    await expect(page.locator('.bottom-nav-item', { hasText: 'Sign in' })).not.toBeVisible();
    // Mobile header shows bell and avatar chip
    await expect(page.locator('.mobile-header-btn')).toBeVisible();
    await expect(page.locator('.mobile-header-avatar')).toBeVisible();

    await page.screenshot({ path: ss('02-auth-bottom-nav.png'), fullPage: false });
  });

  test('bottom nav Feed link navigates to /home', async ({ page }) => {
    await page.goto(`${WEB}/people`);
    await waitForHydration(page);

    await page.locator('.bottom-nav-item', { hasText: 'Feed' }).click();
    await expect(page).toHaveURL(new RegExp('/home'));
  });

  test('bottom nav People link navigates to /people', async ({ page }) => {
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);
    await page.locator('.page-content').waitFor({ state: 'visible', timeout: 10_000 });

    await page.locator('.bottom-nav-item', { hasText: 'People' }).click();
    await expect(page).toHaveURL(new RegExp('/people'));
  });

  test('bottom nav Clubs link navigates to /clubs', async ({ page }) => {
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);
    await page.locator('.page-content').waitFor({ state: 'visible', timeout: 10_000 });

    await page.locator('.bottom-nav-item', { hasText: 'Clubs' }).click();
    await expect(page).toHaveURL(new RegExp('/clubs'));
  });
});

// ---------------------------------------------------------------------------
// Mobile home — guest view
// ---------------------------------------------------------------------------

test.describe('Mobile home — guest', () => {
  test.use({ viewport: MOBILE });

  test('guest sees feed container but no compose card', async ({ page }) => {
    await page.context().clearCookies();
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    await expect(page.locator('.feed')).toBeVisible();
    await expect(page.locator('.compose-card')).not.toBeVisible();
  });

  test('guest sees login gate at bottom of feed', async ({ page }) => {
    await page.context().clearCookies();
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);
    await page.locator('.feed-gate-section').waitFor({ state: 'attached', timeout: 10_000 });

    await expect(page.locator('.feed-gate-card')).toBeVisible();
    await expect(page.locator('.feed-gate-card a', { hasText: 'Sign in' })).toBeVisible();
    await expect(
      page.locator('.feed-gate-card a', { hasText: 'Create account' }),
    ).toBeVisible();

    await page.locator('.feed-gate-section').scrollIntoViewIfNeeded();
    await page.screenshot({ path: ss('03-guest-home-login-gate.png'), fullPage: false });
  });

  test('login gate Sign in link navigates to /login', async ({ page }) => {
    await page.context().clearCookies();
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);
    await page.locator('.feed-gate-section').waitFor({ state: 'attached', timeout: 10_000 });

    await page.locator('.feed-gate-card a', { hasText: 'Sign in' }).click();
    await expect(page).toHaveURL(new RegExp('/login'));
  });

  test('no horizontal overflow on guest home', async ({ page }) => {
    await page.context().clearCookies();
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Mobile home — authenticated
// ---------------------------------------------------------------------------

test.describe('Mobile home — authenticated', () => {
  test.use({ viewport: MOBILE });

  test('authenticated home shows compose card and file drop zone', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/home`);
    await waitForHydration(page);
    await page.locator('.page-content').waitFor({ state: 'visible', timeout: 10_000 });

    await expect(page.locator('.compose-card')).toBeVisible();
    await expect(page.locator('.file-drop-zone')).toBeVisible();
    await expect(page.locator('.file-prompt-text')).toBeVisible();

    await page.screenshot({ path: ss('04-auth-home.png'), fullPage: false });
  });

  test('no horizontal overflow on authenticated home', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/home`);
    await waitForHydration(page);
    await page.locator('.page-content').waitFor({ state: 'visible', timeout: 10_000 });

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Mobile settings
// ---------------------------------------------------------------------------

test.describe('Mobile settings', () => {
  test.use({ viewport: MOBILE });

  test('all four settings sections are visible on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/settings`);
    await waitForHydration(page);
    await page.locator('.settings-section').first().waitFor({ state: 'visible', timeout: 10_000 });

    await expect(page.locator('.settings-section', { hasText: 'Appearance' })).toBeVisible();
    await expect(page.locator('.settings-section', { hasText: 'Privacy' })).toBeVisible();
    await expect(page.locator('.settings-section', { hasText: 'Integrations' })).toBeVisible();
    await expect(page.locator('.settings-danger-zone')).toBeVisible();

    await page.screenshot({ path: ss('05-settings.png'), fullPage: true });
  });

  test('no horizontal overflow on settings page', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/settings`);
    await waitForHydration(page);
    await page.locator('.settings-section').first().waitFor({ state: 'visible', timeout: 10_000 });

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);
  });

  test('theme toggle works on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/settings`);
    await waitForHydration(page);
    await page.locator('.settings-section', { hasText: 'Appearance' }).waitFor({
      state: 'visible',
      timeout: 10_000,
    });

    const beforeTheme = await page.locator('html').getAttribute('data-theme');
    const targetTheme = beforeTheme === 'light' ? 'dark' : 'light';
    const targetCard = page.locator(`[data-testid="theme-option-${targetTheme}"]`);

    await targetCard.click();
    await expect(targetCard).toHaveAttribute('aria-pressed', 'true', { timeout: 8_000 });
    await expect(page.locator('html')).toHaveAttribute('data-theme', targetTheme, { timeout: 5_000 });
  });

  test('public profile privacy toggle works on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/settings`);
    await waitForHydration(page);
    await page.locator('.settings-section', { hasText: 'Privacy' }).waitFor({
      state: 'visible',
      timeout: 10_000,
    });

    const publicToggleRow = page
      .locator('.settings-section', { hasText: 'Privacy' })
      .locator('.toggle-row', { hasText: 'Public profile' });
    const checkbox = publicToggleRow.locator('input[type="checkbox"]');
    const before = await checkbox.isChecked();

    await publicToggleRow.locator('.toggle-switch').click();
    await expect(checkbox).toBeChecked({ checked: !before, timeout: 8_000 });
  });

  test('danger zone delete confirmation prompt is reachable on mobile', async ({
    page,
    request,
  }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/settings`);
    await waitForHydration(page);
    await page.locator('.settings-danger-zone').waitFor({ state: 'visible', timeout: 10_000 });

    await page
      .locator('.settings-danger-zone .btn-danger', { hasText: 'Delete account' })
      .click();
    await expect(page.locator('.danger-confirm')).toBeVisible({ timeout: 5_000 });
    await expect(page.locator('.danger-confirm')).toContainText(`@${auth.username}`);

    // Submit stays disabled until exact handle is typed
    const submitBtn = page.locator('.danger-confirm-actions .btn-danger');
    await page.locator('#delete-confirm').fill('wrong');
    await expect(submitBtn).toBeDisabled();
    await page.locator('#delete-confirm').fill(`@${auth.username}`);
    await expect(submitBtn).not.toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// Mobile post interactions
// ---------------------------------------------------------------------------

test.describe('Mobile post interactions', () => {
  test.use({ viewport: MOBILE });

  test('like button is tappable on mobile', async ({ page, request }) => {
    const suffix = uniqueSuffix();
    const title = `Mobile like test ${suffix}`;
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await uploadExercise(request, auth.token, title);

    await injectAuth(page, auth);
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    const card = page.locator('.feed-card', { hasText: title });
    await card.waitFor({ state: 'visible', timeout: 10_000 });

    const likeBtn = card.locator('.like-btn').first();
    await likeBtn.scrollIntoViewIfNeeded();
    await page.evaluate(() => window.scrollBy(0, 80));
    await expect(likeBtn).toBeVisible();
    const initialCount = await likeBtn.textContent();

    await likeBtn.click();
    await expect(likeBtn).toHaveClass(/like-btn-active/, { timeout: 5_000 });

    const afterCount = await likeBtn.textContent();
    expect(afterCount).not.toEqual(initialCount);
  });

  test('unlike works on mobile', async ({ page, request }) => {
    const suffix = uniqueSuffix();
    const title = `Mobile unlike test ${suffix}`;
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await uploadExercise(request, auth.token, title);

    await injectAuth(page, auth);
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    const card = page.locator('.feed-card', { hasText: title });
    await card.waitFor({ state: 'visible', timeout: 10_000 });
    const likeBtn = card.locator('.like-btn').first();
    await likeBtn.scrollIntoViewIfNeeded();
    await page.evaluate(() => window.scrollBy(0, 80));

    await likeBtn.click();
    await expect(likeBtn).toHaveClass(/like-btn-active/, { timeout: 5_000 });

    await likeBtn.click();
    await expect(likeBtn).not.toHaveClass(/like-btn-active/, { timeout: 5_000 });
  });

  test('tapping a feed card navigates to the exercise detail page', async ({ page, request }) => {
    const suffix = uniqueSuffix();
    const title = `Mobile detail test ${suffix}`;
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await uploadExercise(request, auth.token, title);

    await injectAuth(page, auth);
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    const card = page.locator('.feed-card', { hasText: title });
    await card.waitFor({ state: 'visible', timeout: 10_000 });
    await card.locator('.stats-grid').click();

    await expect(page).toHaveURL(new RegExp(`/@${auth.username}/exercises/`), {
      timeout: 10_000,
    });
    await waitForHydration(page);
    await expect(page.locator('.exercise-detail-card')).toBeVisible({ timeout: 10_000 });
  });

  test('thread divider is visible on exercise detail on mobile', async ({ page, request }) => {
    const suffix = uniqueSuffix();
    const title = `Mobile thread test ${suffix}`;
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await uploadExercise(request, auth.token, title);

    await injectAuth(page, auth);
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    const card = page.locator('.feed-card', { hasText: title });
    await card.waitFor({ state: 'visible', timeout: 10_000 });
    await card.locator('.stats-grid').click();

    await expect(page).toHaveURL(new RegExp(`/@${auth.username}/exercises/`), {
      timeout: 10_000,
    });
    await waitForHydration(page);
    await expect(page.locator('.thread-divider')).toBeVisible({ timeout: 10_000 });
  });

  test('reply composer is visible and focusable on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    const suffix = uniqueSuffix();
    const replyTitle = `Mobile reply test ${suffix}`;
    await uploadExercise(request, auth.token, replyTitle);

    await injectAuth(page, auth);
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    const card = page.locator('.feed-card', { hasText: replyTitle });
    await card.waitFor({ state: 'visible', timeout: 10_000 });
    await card.locator('.stats-grid').click();

    await expect(page).toHaveURL(new RegExp(`/@${auth.username}/exercises/`), {
      timeout: 10_000,
    });
    await waitForHydration(page);

    await expect(page.locator('.reply-composer')).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('.reply-textarea')).toBeVisible();
    await page.locator('.reply-textarea').click();
    await expect(page.locator('.reply-textarea')).toBeFocused();

    await page.screenshot({ path: ss('06-exercise-detail.png'), fullPage: false });
  });

  test('owner post menu (⋯) is tappable on mobile', async ({ page, request }) => {
    const suffix = uniqueSuffix();
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    const menuTitle = `Mobile menu test ${suffix}`;
    await uploadExercise(request, auth.token, menuTitle);

    await injectAuth(page, auth);
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    const card = page.locator('.feed-card', { hasText: menuTitle });
    await card.waitFor({ state: 'visible', timeout: 10_000 });

    await expect(card.locator('.post-menu-trigger')).toBeVisible();
    await card.locator('.post-menu-trigger').click({ force: true });

    await expect(page.locator('.post-menu-dropdown')).toBeVisible({ timeout: 5_000 });
    await expect(page.locator('.post-menu-item', { hasText: 'Edit' })).toBeVisible();
    await expect(
      page.locator('.post-menu-item-danger', { hasText: 'Delete' }),
    ).toBeVisible();
  });

  test('no horizontal overflow on exercise detail page', async ({ page, request }) => {
    const suffix = uniqueSuffix();
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    const title = `Mobile detail overflow test ${suffix}`;
    await uploadExercise(request, auth.token, title);

    await injectAuth(page, auth);
    await page.goto(`${WEB}/home`);
    await waitForHydration(page);

    const card = page.locator('.feed-card', { hasText: title });
    await card.waitFor({ state: 'visible', timeout: 10_000 });
    await card.locator('.stats-grid').click();

    await expect(page).toHaveURL(new RegExp(`/@${auth.username}/exercises/`), {
      timeout: 10_000,
    });
    await waitForHydration(page);

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Mobile profile
// ---------------------------------------------------------------------------

test.describe('Mobile profile', () => {
  test.use({ viewport: MOBILE });

  test('public profile renders profile card on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');

    await page.goto(`${WEB}/@${auth.username}`);
    await waitForHydration(page);

    await expect(page.locator('.profile-card')).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('.profile-card')).toContainText(`@${auth.username}`);
    await expect(page.locator('.profile-handle')).toContainText(`@${auth.username}@`);
  });

  test('own profile shows Edit profile button on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/@${auth.username}`);
    await waitForHydration(page);
    await page.locator('.profile-card').waitFor({ state: 'visible', timeout: 10_000 });

    await expect(page.locator('.profile-edit-btn')).toBeVisible();
  });

  test('profile edit form opens, saves, and closes on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    const newName = `Mobile Athlete ${uniqueSuffix()}`;
    await injectAuth(page, auth);

    await page.goto(`${WEB}/@${auth.username}`);
    await waitForHydration(page);
    await page.locator('.profile-card').waitFor({ state: 'visible', timeout: 10_000 });

    await page.locator('.profile-edit-btn').click();
    await page.locator('.profile-edit-form').waitFor({ state: 'visible', timeout: 5_000 });

    await expect(page.locator('.profile-edit-form input[type="text"]')).toBeVisible();
    await expect(page.locator('.profile-edit-form textarea')).toBeVisible();
    await expect(page.locator('.profile-edit-form .btn-primary')).toHaveText('Save');
    await expect(page.locator('.profile-edit-form .btn-ghost')).toHaveText('Cancel');

    await page.locator('.profile-edit-form input[type="text"]').fill(newName);
    await page.locator('.profile-edit-form .btn-primary').click();

    await expect(page.locator('.profile-edit-form')).not.toBeVisible({ timeout: 10_000 });
    await expect(page.locator('.profile-name')).toContainText(newName, { timeout: 10_000 });
  });

  test('profile edit cancel discards changes on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/@${auth.username}`);
    await waitForHydration(page);
    await page.locator('.profile-card').waitFor({ state: 'visible', timeout: 10_000 });

    await page.locator('.profile-edit-btn').click();
    await page.locator('.profile-edit-form').waitFor({ state: 'visible', timeout: 5_000 });

    await page.locator('.profile-edit-form input[type="text"]').fill('Should not be saved');
    await page.locator('.profile-edit-form .btn-ghost').click();

    await expect(page.locator('.profile-edit-form')).not.toBeVisible();
    await expect(page.locator('.profile-edit-btn')).toBeVisible();
  });

  test('connections modal opens from profile stats on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);

    await page.goto(`${WEB}/@${auth.username}`);
    await waitForHydration(page);
    await page.locator('.profile-card').waitFor({ state: 'visible', timeout: 10_000 });

    await page.locator('.profile-stat', { hasText: 'Following' }).click();
    await expect(page.locator('.connections-modal')).toBeVisible({ timeout: 5_000 });
    await expect(page.locator('.modal-tab-active')).toContainText('Following');

    await page.screenshot({ path: ss('07-connections-modal-mobile.png'), fullPage: false });

    await page.locator('.modal-close').click();
    await expect(page.locator('.connections-modal')).not.toBeVisible();
  });

  test('no horizontal overflow on profile page', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');

    await page.goto(`${WEB}/@${auth.username}`);
    await waitForHydration(page);
    await page.locator('.profile-card').waitFor({ state: 'visible', timeout: 10_000 });

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);
  });

  test('profile page screenshot on mobile', async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');

    await page.goto(`${WEB}/@${auth.username}`);
    await waitForHydration(page);
    await page.locator('.profile-card').waitFor({ state: 'visible', timeout: 10_000 });
    await page
      .locator('.feed-card, .profile-empty-posts')
      .first()
      .waitFor({ state: 'visible', timeout: 10_000 });
    await page.waitForTimeout(300);

    await page.screenshot({ path: ss('08-profile-page.png'), fullPage: false });
  });
});

// ---------------------------------------------------------------------------
// Mobile auth pages
// ---------------------------------------------------------------------------

test.describe('Mobile auth pages', () => {
  test.use({ viewport: MOBILE });

  test('login form fields are visible and page has no overflow', async ({ page }) => {
    await page.goto(`${WEB}/login`);
    await waitForHydration(page);

    await expect(page.locator('.auth-card')).toBeVisible();
    // Jogga login uses a combined login field (email or username)
    await expect(page.locator('input[autocomplete="username"], input[id="login-field"]')).toBeVisible();
    await expect(page.locator('input[autocomplete="current-password"]')).toBeVisible();
    await expect(page.locator('button:has-text("Sign in")')).toBeVisible();

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);

    await page.screenshot({ path: ss('09-login.png'), fullPage: false });
  });

  test('register form fields are visible and page has no overflow', async ({ page }) => {
    await page.goto(`${WEB}/register`);
    await waitForHydration(page);

    await expect(page.locator('.auth-card')).toBeVisible();
    await expect(page.locator('input[autocomplete="username"]')).toBeVisible();
    await expect(page.locator('button:has-text("Send verification code")')).toBeVisible();
    await expect(page.locator('a:has-text("Sign in")')).toBeVisible();

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);

    await page.screenshot({ path: ss('10-register.png'), fullPage: false });
  });
});

// ---------------------------------------------------------------------------
// Mobile People and Clubs pages
// ---------------------------------------------------------------------------

test.describe('Mobile People and Clubs pages', () => {
  test.use({ viewport: MOBILE });

  test.beforeEach(async ({ page, request }) => {
    const auth = await createUser(request, 'owner', 'owner@jogga.test', 'testpass99');
    await injectAuth(page, auth);
  });

  test('People page renders without horizontal overflow', async ({ page }) => {
    await page.goto(`${WEB}/people`);
    await waitForHydration(page);
    await expect(page.locator('.settings-title')).toHaveText('People');

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);

    await page.screenshot({ path: ss('11-people-page.png'), fullPage: false });
  });

  test('Clubs page renders without horizontal overflow', async ({ page }) => {
    await page.goto(`${WEB}/clubs`);
    await waitForHydration(page);
    await page.locator('.page-content').waitFor({ state: 'visible', timeout: 10_000 });

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    );
    expect(overflow).toBe(false);

    await page.screenshot({ path: ss('12-clubs-page.png'), fullPage: false });
  });
});
