/**
 * Federation — remote club (Group actor) interaction
 *
 * Verifies that a single-owner Jogga server can federate with a Group actor
 * hosted on a remote fedisport-compatible server.
 *
 * Architecture:
 *   JOGGA        BASE_URL        (localhost:6060)  — pre-seeded owner, single-user
 *   CLUB SERVER  CLUB_SERVER_URL (localhost:8080)  — fedisport with clubs enabled
 *
 * Key differences from fedisport's own club tests:
 *   - No user registration on the Jogga side; owner is pre-seeded and persistent.
 *   - No DB truncation; cleanup is unfollow-only (owner account can't be recreated).
 *   - RUN suffix applied to remote club handles (not local username) to isolate runs.
 *   - Follow acceptance verified via notifications API (kind: "follow_accepted")
 *     and the REST following list, rather than UI polling.
 *
 * Prerequisites:
 *   - Jogga server at BASE_URL with seeded owner (OWNER_EMAIL / OWNER_PASSWORD)
 *   - Fedisport server at CLUB_SERVER_URL with dev.otp_echo = true
 *     OR set CLUB_ADMIN_TOKEN to skip admin user creation
 *
 * Run:
 *   INTEGRATION_TEST=true CLUB_SERVER_URL=http://localhost:8080 npm test -- --grep "club"
 *
 * With pre-existing admin:
 *   INTEGRATION_TEST=true CLUB_SERVER_URL=http://localhost:8080 \
 *     CLUB_ADMIN_TOKEN=<token> npm test -- --grep "club"
 */

import { test, expect, APIRequestContext } from '@playwright/test';
import {
  getOwnerProfile,
  apiPost,
  apiGet,
  sleep,
  injectAuth,
  waitForHydration,
  fedisportCreateUser,
  fedisportCreateClub,
  fedisportClubMemberAction,
  fedisportDeleteUser,
  resolveClubActorId,
} from '../fixtures/helpers';
import {
  WEB,
  OWNER_EMAIL,
  OWNER_PASSWORD,
  CLUB_SERVER_URL,
  CLUB_SERVER_DOMAIN,
  CLUB_ADMIN_TOKEN,
  SKIP_CLUB_FED,
} from '../fixtures/env';

test.skip(SKIP_CLUB_FED, 'set INTEGRATION_TEST=true and CLUB_SERVER_URL to run club federation spec');
test.describe.configure({ mode: 'serial' });
test.setTimeout(120_000);

// ── per-run identifiers ───────────────────────────────────────────────────────

const RUN = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);
const ADMIN_USERNAME  = `adm_${RUN}`;
const OPEN_HANDLE     = `openclub_${RUN}`;
const EXCL_HANDLE     = `exclclub_${RUN}`;
const AP_ACCEPT = 'application/activity+json';

// ── shared test state (populated in beforeAll / setup tests) ──────────────────

interface Owner { token: string; username: string; ap_id: string; }
interface Admin { token: string; username: string; ap_id: string; }

let owner: Owner;
let admin: Admin;
let openClubApId: string;
let exclClubApId: string;

// ── beforeAll: resolve owner + spin up remote clubs ───────────────────────────

test.beforeAll(async ({ request }) => {
  owner = await getOwnerProfile(request, OWNER_EMAIL, OWNER_PASSWORD);

  if (CLUB_ADMIN_TOKEN) {
    // Use pre-existing admin — get ap_id from the server
    const me = await request.get(`${CLUB_SERVER_URL}/api/v1/accounts/me`, {
      headers: { Authorization: `Bearer ${CLUB_ADMIN_TOKEN}` },
    });
    if (!me.ok()) throw new Error(`CLUB_ADMIN_TOKEN invalid: ${me.status()}`);
    const body = await me.json();
    admin = { token: CLUB_ADMIN_TOKEN, username: body.username, ap_id: body.ap_id };
  } else {
    admin = await fedisportCreateUser(
      request, ADMIN_USERNAME, `${ADMIN_USERNAME}@example.test`, 'TestPass99!', CLUB_SERVER_URL,
    );
  }

  await fedisportCreateClub(request, admin.token, OPEN_HANDLE, 'Open Club', false, CLUB_SERVER_URL);
  await fedisportCreateClub(request, admin.token, EXCL_HANDLE, 'Exclusive Club', true, CLUB_SERVER_URL);

  // Resolve canonical AP IDs via WebFinger so tests don't hard-code URL structure
  [openClubApId, exclClubApId] = await Promise.all([
    resolveClubActorId(request, OPEN_HANDLE, CLUB_SERVER_DOMAIN, CLUB_SERVER_URL),
    resolveClubActorId(request, EXCL_HANDLE, CLUB_SERVER_DOMAIN, CLUB_SERVER_URL),
  ]);

  console.log(`owner:        ${owner.ap_id}`);
  console.log(`admin:        ${admin.ap_id}`);
  console.log(`open club:    ${openClubApId}`);
  console.log(`excl club:    ${exclClubApId}`);
});

// ── afterAll: clean up follow relationships + remote admin user ───────────────

test.afterAll(async ({ request }) => {
  for (const handle of [OPEN_HANDLE, EXCL_HANDLE]) {
    try {
      await apiPost(request, '/api/v1/unfollow', { target: `@${handle}@${CLUB_SERVER_DOMAIN}` }, owner.token);
    } catch { /* already unfollowed or never followed */ }
  }
  // Remove admin from club server (only if we created it ourselves)
  if (!CLUB_ADMIN_TOKEN) {
    await fedisportDeleteUser(request, admin.token, CLUB_SERVER_URL);
  }
});

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Club actor discovery
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('club actor discovery', () => {
  test('WebFinger for open club resolves with self link', async ({ request }) => {
    const res = await request.get(
      `${CLUB_SERVER_URL}/.well-known/webfinger?resource=acct:${OPEN_HANDLE}@${CLUB_SERVER_DOMAIN}`,
    );
    expect(res.status()).toBe(200);
    const jrd = await res.json();
    expect(jrd).toHaveProperty('subject');
    expect(
      (jrd.links as Array<{ rel: string }>).some(l => l.rel === 'self'),
      'JRD must include a self link',
    ).toBe(true);
  });

  test('AP actor for open club has type Group', async ({ request }) => {
    const res = await request.get(openClubApId, { headers: { Accept: AP_ACCEPT } });
    expect(res.status()).toBe(200);
    const actor = await res.json();
    expect(actor.type).toBe('Group');
    expect(actor).toHaveProperty('inbox');
    expect(actor).toHaveProperty('followers');
    expect(actor.preferredUsername).toBe(OPEN_HANDLE);
  });

  test('AP actor for exclusive club has type Group and manually approves followers', async ({ request }) => {
    const res = await request.get(exclClubApId, { headers: { Accept: AP_ACCEPT } });
    expect(res.status()).toBe(200);
    const actor = await res.json();
    expect(actor.type).toBe('Group');
    expect(actor.manuallyApprovesFollowers).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Open club: Follow → Accept round-trip
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('open club: follow + Accept round-trip', () => {
  test('owner sends Follow to open club', async ({ request }) => {
    const res = await apiPost(
      request,
      '/api/v1/follows',
      { target: `@${OPEN_HANDLE}@${CLUB_SERVER_DOMAIN}` },
      owner.token,
    );
    expect(res.status()).toBe(202);
  });

  test('Accept delivered: open club appears as accepted in following list', async ({ request }) => {
    // Open clubs auto-accept; poll until Accept round-trip completes
    await expect.poll(
      async () => {
        const res = await apiGet(request, '/api/v1/accounts/me/following', 'application/json', owner.token);
        const list = await res.json() as Array<{ ap_id: string; accepted: boolean }>;
        const entry = list.find(f => f.ap_id === openClubApId);
        if (!entry) return 'missing';
        return entry.accepted ? 'accepted' : 'pending';
      },
      { timeout: 30_000, message: 'open club follow should be accepted' },
    ).toBe('accepted');
  });

  test('Accept delivered: follow_accepted notification exists for open club', async ({ request }) => {
    // When core's inbox receives Accept, it inserts a follow_accepted notification
    await expect.poll(
      async () => {
        const res = await apiGet(request, '/api/v1/notifications', 'application/json', owner.token);
        const body = await res.json() as { notifications: Array<{ kind: string; from_ap_id: string }> };
        return body.notifications.some(
          n => n.kind === 'follow_accepted' && n.from_ap_id === openClubApId,
        );
      },
      { timeout: 30_000, message: 'follow_accepted notification from open club' },
    ).toBe(true);
  });

  test('AP following collection on Jogga reflects new follow', async ({ request }) => {
    const res = await request.get(`${WEB}/users/${owner.username}/following`, {
      headers: { Accept: AP_ACCEPT },
    });
    expect(res.status()).toBe(200);
    const collection = await res.json();
    expect(collection.totalItems).toBeGreaterThanOrEqual(1);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Club post delivery: club Announces → owner inbox
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('open club: post delivery', () => {
  const EXERCISE_TITLE = `Club federation test [${RUN}]`;
  let exerciseApId: string | undefined;

  test('club admin uploads exercise on the club server', async ({ request }) => {
    const gpx = `<?xml version="1.0"?><gpx version="1.1" creator="test"><trk>
      <name>${EXERCISE_TITLE}</name>
      <trkseg>
        <trkpt lat="48.8566" lon="2.3522"><ele>35</ele><time>2024-01-01T08:00:00Z</time></trkpt>
        <trkpt lat="48.8570" lon="2.3530"><ele>36</ele><time>2024-01-01T08:01:00Z</time></trkpt>
      </trkseg></trk></gpx>`;

    const res = await request.post(`${CLUB_SERVER_URL}/api/exercises/upload`, {
      headers: { Authorization: `Bearer ${admin.token}` },
      multipart: {
        activityType: 'run',
        title: EXERCISE_TITLE,
        file: { name: 'test.gpx', mimeType: 'application/gpx+xml', buffer: Buffer.from(gpx) },
      },
    });
    expect(res.status(), 'exercise upload on club server').toBe(201);
    const body = await res.json();
    exerciseApId = (body.ap_id ?? body.id) as string | undefined;
    console.log(`exercise ap_id: ${exerciseApId}`);
  });

  // fedisport's "boost to club" (share_to_club) is implemented as a Dioxus server
  // function, not a REST API endpoint — there is no /api/v1/clubs/{handle}/boost.
  // Triggering it from a test would require either:
  //   a) A dedicated REST endpoint added to fedisport, or
  //   b) Sending a signed ActivityPub Create to the club inbox directly (HTTP sig generation)
  // TODO: add POST /api/v1/clubs/:handle/boost to fedisport, then enable these tests.
  test.skip('admin shares exercise to the open club (boost)', async ({ request }) => {
    const res = await request.post(`${CLUB_SERVER_URL}/api/v1/clubs/${OPEN_HANDLE}/boost`, {
      headers: { Authorization: `Bearer ${admin.token}`, 'Content-Type': 'application/json' },
      data: { object_id: exerciseApId },
    });
    expect([200, 201, 202, 204]).toContain(res.status());
  });

  test.skip('club AP outbox reflects the Announce activity', async ({ request }) => {
    await sleep(8_000);
    const res = await request.get(`${openClubApId}/outbox`, { headers: { Accept: AP_ACCEPT } });
    if (!res.ok()) return; // outbox may require auth
    const outbox = await res.json();
    expect(outbox.totalItems ?? (outbox.orderedItems?.length ?? 0)).toBeGreaterThanOrEqual(1);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Exclusive club: Follow → pending → admin accepts
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('exclusive club: request + admin accept', () => {
  test('owner sends Follow to exclusive club', async ({ request }) => {
    const res = await apiPost(
      request,
      '/api/v1/follows',
      { target: `@${EXCL_HANDLE}@${CLUB_SERVER_DOMAIN}` },
      owner.token,
    );
    expect(res.status()).toBe(202);
  });

  test('exclusive club follow is pending (not auto-accepted)', async ({ request }) => {
    // Give the club server time to receive the Follow and record it as a request
    await sleep(4_000);

    const res = await apiGet(request, '/api/v1/accounts/me/following', 'application/json', owner.token);
    const list = await res.json() as Array<{ ap_id: string; accepted: boolean }>;
    const entry = list.find(f => f.ap_id === exclClubApId);
    expect(entry, 'exclusive club should appear in following list').toBeTruthy();
    expect(entry!.accepted, 'exclusive club follow should be pending').toBe(false);
  });

  test('admin accepts owner — follow becomes accepted', async ({ request }) => {
    await fedisportClubMemberAction(
      request, admin.token, EXCL_HANDLE, owner.ap_id, 'accept', CLUB_SERVER_URL,
    );

    // Poll until Accept round-trip completes and Jogga marks the follow as accepted
    await expect.poll(
      async () => {
        const res = await apiGet(request, '/api/v1/accounts/me/following', 'application/json', owner.token);
        const list = await res.json() as Array<{ ap_id: string; accepted: boolean }>;
        return list.find(f => f.ap_id === exclClubApId)?.accepted ?? false;
      },
      { timeout: 30_000, message: 'exclusive club follow should become accepted after admin Accept' },
    ).toBe(true);
  });

  test('follow_accepted notification from exclusive club', async ({ request }) => {
    await expect.poll(
      async () => {
        const res = await apiGet(request, '/api/v1/notifications', 'application/json', owner.token);
        const body = await res.json() as { notifications: Array<{ kind: string; from_ap_id: string }> };
        return body.notifications.some(
          n => n.kind === 'follow_accepted' && n.from_ap_id === exclClubApId,
        );
      },
      { timeout: 30_000, message: 'follow_accepted notification from exclusive club' },
    ).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// 5. UI smoke: owner's home page loads while following clubs
// ═══════════════════════════════════════════════════════════════════════════════

test('UI: owner home page loads with club follows active', async ({ page }) => {
  await injectAuth(page, owner);
  await page.goto(`${WEB}/home`);
  await waitForHydration(page);
  // Basic smoke — the feed shell renders without error when following Group actors
  await expect(page.locator('body[data-hydrated]')).toBeAttached();
});

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Cleanup: unfollow both clubs
// ═══════════════════════════════════════════════════════════════════════════════

test.describe('cleanup: unfollow both clubs', () => {
  for (const [label, handle, getApId] of [
    ['open',      OPEN_HANDLE, () => openClubApId] as const,
    ['exclusive', EXCL_HANDLE, () => exclClubApId] as const,
  ]) {
    test(`owner unfollows ${label} club`, async ({ request }) => {
      const res = await apiPost(
        request,
        '/api/v1/unfollow',
        { target: `@${handle}@${CLUB_SERVER_DOMAIN}` },
        owner.token,
      );
      expect([200, 202, 204]).toContain(res.status());

      await expect.poll(
        async () => {
          const r = await apiGet(request, '/api/v1/accounts/me/following', 'application/json', owner.token);
          const list = await r.json() as Array<{ ap_id: string }>;
          return list.some(f => f.ap_id === getApId());
        },
        { timeout: 15_000, message: `${label} club should be removed from following` },
      ).toBe(false);
    });
  }
});
