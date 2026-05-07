/**
 * Direct API tests using Playwright's request context.
 *
 * Most tests require a running server with a seeded owner account and are
 * skipped unless INTEGRATION_TEST=true is set.
 *
 * Run:
 *   INTEGRATION_TEST=true npm test -- --grep "API"
 */
import { test, expect, APIRequestContext, request as pwRequest } from '@playwright/test';
import { SKIP_INTEGRATION } from '../fixtures/env';

const BASE_URL = process.env.BASE_URL ?? 'http://localhost:6060';
const AP_ACCEPT = 'application/activity+json';
const OWNER_USERNAME = process.env.OWNER_USERNAME || 'owner';

let ctx: APIRequestContext;

test.beforeAll(async () => {
  ctx = await pwRequest.newContext({ baseURL: BASE_URL });
});

test.afterAll(async () => {
  await ctx.dispose();
});

// ---------------------------------------------------------------------------
// Actor
// ---------------------------------------------------------------------------

test.describe('API — GET /users/:username', () => {
  test('returns ActivityPub Person for owner', async () => {
    test.skip(SKIP_INTEGRATION, 'set INTEGRATION_TEST=true for this test');

    const res = await ctx.get(`/users/${OWNER_USERNAME}`, {
      headers: { Accept: AP_ACCEPT },
    });
    expect(res.status()).toBe(200);
    expect(res.headers()['content-type']).toMatch(/activity\+json/);

    const body = await res.json();
    expect(body.type).toBe('Person');
    expect(body.preferredUsername).toBe(OWNER_USERNAME);
    expect(body).toHaveProperty('inbox');
  });

  test('returns 404 for non-existent user', async () => {
    const res = await ctx.get('/users/nobody', {
      headers: { Accept: AP_ACCEPT },
    });
    expect(res.status()).toBe(404);
  });
});

// ---------------------------------------------------------------------------
// NodeInfo
// ---------------------------------------------------------------------------

test.describe('API — GET /.well-known/nodeinfo', () => {
  test('returns well-known nodeinfo discovery doc', async () => {
    test.skip(SKIP_INTEGRATION, 'set INTEGRATION_TEST=true for this test');

    const res = await ctx.get('/.well-known/nodeinfo');
    expect(res.status()).toBe(200);

    const body = await res.json();
    expect(Array.isArray(body.links)).toBeTruthy();
    expect(body.links.length).toBeGreaterThan(0);
  });
});

test.describe('API — GET /nodeinfo/2.1', () => {
  test('returns nodeinfo with software name', async () => {
    test.skip(SKIP_INTEGRATION, 'set INTEGRATION_TEST=true for this test');

    const res = await ctx.get('/nodeinfo/2.1');
    expect(res.status()).toBe(200);

    const body = await res.json();
    expect(body).toHaveProperty('version');
    expect(body).toHaveProperty('software');
    expect(body.software).toHaveProperty('name');
  });
});

// ---------------------------------------------------------------------------
// WebFinger
// ---------------------------------------------------------------------------

test.describe('API — GET /.well-known/webfinger', () => {
  test('returns webfinger JRD for owner account', async () => {
    test.skip(SKIP_INTEGRATION, 'set INTEGRATION_TEST=true for this test');

    const domain = new URL(BASE_URL).host; // includes port, e.g. localhost:6060
    const res = await ctx.get(
      `/.well-known/webfinger?resource=acct:${OWNER_USERNAME}@${domain}`,
    );
    expect(res.status()).toBe(200);

    const body = await res.json();
    expect(body).toHaveProperty('subject');
    expect(Array.isArray(body.links)).toBeTruthy();
    expect(
      body.links.some((l: { rel: string }) => l.rel === 'self'),
    ).toBeTruthy();
  });

  test('returns 404 for unknown account', async () => {
    test.skip(SKIP_INTEGRATION, 'set INTEGRATION_TEST=true for this test');

    const domain = new URL(BASE_URL).host;
    const res = await ctx.get(
      `/.well-known/webfinger?resource=acct:nobody_xyz@${domain}`,
    );
    expect(res.status()).toBe(404);
  });
});
