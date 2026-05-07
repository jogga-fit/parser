import type { APIRequestContext, BrowserContext, Page } from '@playwright/test';

const DEFAULT_BASE = process.env.BASE_URL ?? 'http://localhost:6060';

/**
 * Wait for the Dioxus WASM to finish hydrating the page.
 * 60 s gives headroom under load (cold WASM can take 15-25 s).
 */
export async function waitForHydration(page: Page, timeout = 60_000): Promise<void> {
  await page.locator('body[data-hydrated]').waitFor({ state: 'attached', timeout });
}

/**
 * Log in via the Jogga UI form and wait for redirect to /home.
 * Uses the contact field (email or phone) rather than a separate username field.
 */
export async function loginViaUI(
  page: Page,
  contact: string,
  password: string,
  webUrl = DEFAULT_BASE,
): Promise<void> {
  await page.goto(`${webUrl}/login`);
  await page.locator('body[data-hydrated]').waitFor({ state: 'attached', timeout: 60_000 });
  await page.locator('#login-field').fill(contact);
  await page.locator('input[autocomplete="current-password"]').fill(password);
  await page.locator('button:has-text("Sign in")').click();
  await page.waitForURL(`${webUrl}/home`, { timeout: 60_000 });
}

/**
 * Obtain a bearer token for the owner account via POST /api/v1/accounts/token.
 * Returns the token string on success, throws on failure.
 */
export async function getOwnerToken(
  request: APIRequestContext,
  email: string,
  password: string,
  baseUrl = DEFAULT_BASE,
): Promise<string> {
  const res = await request.post(`${baseUrl}/api/v1/accounts/token`, {
    headers: { 'Content-Type': 'application/json' },
    data: { login: email, password },
  });
  if (!res.ok()) {
    throw new Error(`getOwnerToken failed: ${res.status()} ${await res.text()}`);
  }
  const body = await res.json();
  return body.token as string;
}

/** POST wrapper with optional Bearer token. */
export async function apiPost(
  request: APIRequestContext,
  path: string,
  body?: object,
  token?: string,
  baseUrl = DEFAULT_BASE,
) {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  return request.post(`${baseUrl}${path}`, { headers, data: body ?? {} });
}

/** GET wrapper with optional Bearer token and Accept header. */
export async function apiGet(
  request: APIRequestContext,
  path: string,
  accept = 'application/json',
  token?: string,
  baseUrl = DEFAULT_BASE,
) {
  const headers: Record<string, string> = { Accept: accept };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  return request.get(`${baseUrl}${path}`, { headers });
}

/** Block Google Fonts requests so screenshots are deterministic and font loads don't stall. */
export async function blockExternalFonts(context: BrowserContext): Promise<void> {
  await context.route('https://fonts.googleapis.com/**', r => r.fulfill({ contentType: 'text/css', body: '' }));
  await context.route('https://fonts.gstatic.com/**', r => r.fulfill({ contentType: 'font/woff2', body: '' }));
}
