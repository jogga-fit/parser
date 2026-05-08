import type { APIRequestContext, BrowserContext, Page } from '@playwright/test';

const DEFAULT_BASE = process.env.BASE_URL ?? 'http://localhost:6060';
const OWNER_EMAIL = process.env.OWNER_EMAIL ?? 'owner@jogga.test';
const OWNER_PASSWORD = process.env.OWNER_PASSWORD ?? 'testpass99';

/**
 * Returns a suffix unique across concurrent workers and quick re-runs.
 * Combines a millisecond timestamp with 5 random base-36 chars.
 */
export function uniqueSuffix(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}

/**
 * Jogga is a single-owner server — "create user" returns the owner's credentials.
 * Username/email params are accepted for API compatibility with fedisport helpers
 * but the owner credentials from env vars are always used.
 *
 * Requires INTEGRATION_TEST=true and a running server with a seeded owner account.
 */
export async function createUser(
  request: APIRequestContext,
  _username: string,
  _email: string,
  _password = 'testpass99',
  baseUrl = DEFAULT_BASE,
): Promise<{ token: string; ap_id: string; username: string }> {
  return getOwnerProfile(request, OWNER_EMAIL, OWNER_PASSWORD, baseUrl);
}

export function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}

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

// ─── Owner profile ────────────────────────────────────────────────────────────

/**
 * Fetch the owner's token + full profile (including ap_id) in one call.
 * TokenResponse only returns {token, username}; ap_id requires a second GET /api/v1/accounts/me.
 */
export async function getOwnerProfile(
  request: APIRequestContext,
  email: string,
  password: string,
  baseUrl = DEFAULT_BASE,
): Promise<{ token: string; username: string; ap_id: string }> {
  const token = await getOwnerToken(request, email, password, baseUrl);
  const res = await apiGet(request, '/api/v1/accounts/me', 'application/json', token, baseUrl);
  if (!res.ok()) throw new Error(`getOwnerProfile: GET /me failed ${res.status()}`);
  const body = await res.json();
  return { token, username: body.username as string, ap_id: body.ap_id as string };
}

// ─── UI auth injection ────────────────────────────────────────────────────────

/**
 * Inject owner auth into localStorage before navigating.
 * Core uses the same "fedisport_auth" key as fedisport.
 * Call this before page.goto() so the script runs before page load.
 */
export async function injectAuth(
  page: Page,
  user: { token: string; username: string; ap_id: string },
): Promise<void> {
  await page.addInitScript((u) => {
    localStorage.setItem('fedisport_auth', JSON.stringify(u));
  }, user);
}

// ─── Fedisport-compatible remote server helpers ───────────────────────────────

/**
 * Register a user on a fedisport server.
 * Requires dev.otp_echo = true so the OTP code is returned in the init response.
 */
export async function fedisportCreateUser(
  request: APIRequestContext,
  username: string,
  email: string,
  password: string,
  baseUrl: string,
): Promise<{ token: string; username: string; ap_id: string }> {
  const init = await request.post(`${baseUrl}/api/v1/accounts/register/init`, {
    headers: { 'Content-Type': 'application/json' },
    data: { username, email },
  });
  if (!init.ok()) throw new Error(`fedisport register/init: ${init.status()} ${await init.text()}`);
  const { otp_id, code } = await init.json();

  const verify = await request.post(`${baseUrl}/api/v1/accounts/otp/verify`, {
    headers: { 'Content-Type': 'application/json' },
    data: { otp_id, code, password },
  });
  if (!verify.ok()) throw new Error(`fedisport otp/verify: ${verify.status()} ${await verify.text()}`);
  const body = await verify.json();
  return { token: body.token as string, username: body.username as string, ap_id: body.ap_id as string };
}

/** Create a club on a fedisport server. */
export async function fedisportCreateClub(
  request: APIRequestContext,
  token: string,
  handle: string,
  displayName: string,
  exclusive: boolean,
  baseUrl: string,
): Promise<void> {
  const res = await request.post(`${baseUrl}/api/v1/clubs`, {
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    data: { handle, display_name: displayName, exclusive },
  });
  if (!res.ok()) throw new Error(`fedisport createClub(${handle}): ${res.status()} ${await res.text()}`);
}

/** Accept or reject a membership request on a fedisport club. */
export async function fedisportClubMemberAction(
  request: APIRequestContext,
  token: string,
  handle: string,
  memberApId: string,
  action: 'accept' | 'reject',
  baseUrl: string,
): Promise<void> {
  const encoded = encodeURIComponent(memberApId);
  const res = await request.post(`${baseUrl}/api/v1/clubs/${handle}/members/${encoded}/${action}`, {
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
  });
  if (res.status() !== 204) {
    throw new Error(`fedisport club ${action}(${memberApId}): ${res.status()} ${await res.text()}`);
  }
}

/** Delete a user account on a fedisport server (test cleanup). */
export async function fedisportDeleteUser(
  request: APIRequestContext,
  token: string,
  baseUrl: string,
): Promise<void> {
  await request.delete(`${baseUrl}/api/v1/accounts/me`, {
    headers: { Authorization: `Bearer ${token}` },
  });
}

/**
 * Resolve a club actor's canonical AP ID via WebFinger.
 * Falls back to ${serverUrl}/clubs/${handle} if WebFinger lookup fails.
 */
export async function resolveClubActorId(
  request: APIRequestContext,
  handle: string,
  domain: string,
  serverUrl: string,
): Promise<string> {
  try {
    const res = await request.get(
      `${serverUrl}/.well-known/webfinger?resource=acct:${handle}@${domain}`,
    );
    if (res.ok()) {
      const jrd = await res.json();
      const self = (jrd.links as Array<{ rel: string; href: string }>)
        ?.find(l => l.rel === 'self');
      if (self?.href) return self.href;
    }
  } catch { /* fall through */ }
  return `${serverUrl}/clubs/${handle}`;
}
