# Jogga E2E Test Suite

This directory contains the Playwright suite for user-facing behavior, API smoke
coverage, federation flows, screenshots, responsive checks, and accessibility
checks.

The suite is a mix of:

- **Static/live frontend smoke tests** that only need the app served on `6060`.
- **Single-instance integration tests** that require `INTEGRATION_TEST=true`, a
  live server on `6060`, and a seeded owner account.
- **Federation integration tests** that require a running Jogga server and a
  fedisport-compatible club server.
- **Screenshot/snapshot tests** that produce Playwright report artifacts or
  save reference screenshots to `e2e/screenshots/`.

Do not rewrite, update, or delete Playwright tests because of a UI or UX change
without explicit user instruction. If the UI changed intentionally and a test
fails, report the failure and let the user decide how the test should change.

## Commands

From this directory:

```bash
npm test
npm run test:headed
npm run test:ui
npm run test:debug
npm run report
```

Targeted examples:

```bash
INTEGRATION_TEST=true npx playwright test mobile-ux --project=chromium
INTEGRATION_TEST=true npx playwright test navigation --project=chromium
INTEGRATION_TEST=true npx playwright test api --project=chromium
```

Preferred local integration setup:

```bash
# Build the WASM frontend
dx build

# Seed the owner account (first run only)
cargo run -- seed-owner --username owner --password testpass99

# Start the server (config with dev mode, port 6060)
cargo run -- --config config.toml serve

# In a second terminal, run the tests
cd e2e
INTEGRATION_TEST=true npm test
```

Useful environment variables:

- `BASE_URL`: web URL for single-instance tests, default `http://localhost:6060`.
- `INTEGRATION_TEST`: set to any truthy value to un-skip live-server tests.
- `OWNER_EMAIL`: owner account email, default `owner@jogga.test`.
- `OWNER_PASSWORD`: owner account password, default `testpass99`.
- `CLUB_SERVER_URL`: URL of a fedisport-compatible club server for federation tests.
- `CLUB_SERVER_DOMAIN`: domain of the club server (derived from `CLUB_SERVER_URL` if not set).
- `CLUB_ADMIN_TOKEN`: pre-existing admin token for club server (skips registration when set).

## Local Setup Assumptions

Single-instance tests generally assume:

- `http://localhost:6060` is serving the Dioxus app.
- The owner account is seeded (`cargo run -- seed-owner`).
- Debug build — OTP codes are echoed in the response body automatically.

**Jogga is a single-owner server.** All authenticated E2E tests run as the
owner account. There is no multi-user registration API. The `createUser` helper
in `fixtures/helpers.ts` returns the owner's credentials from env vars.

## Fixtures And Helpers

`fixtures/api-mocks.ts` provides a custom `test` fixture that blocks external
Google font requests so screenshots are deterministic.

`fixtures/helpers.ts` provides common live-server helpers:

- `createUser`: returns the owner's token (single-owner adapter — username/email params ignored).
- `loginViaUI`: fills the login form and waits for `/home`.
- `getOwnerToken`: obtains the owner's bearer token via `POST /api/v1/accounts/token`.
- `getOwnerProfile`: fetches token + full profile (including ap_id).
- `injectAuth`: injects `fedisport_auth` into localStorage before navigation.
- `apiPost` and `apiGet`: small authenticated request wrappers.
- `uniqueSuffix`: collision-resistant suffix for exercise titles in parallel tests.
- `blockExternalFonts`: route-level Google Fonts blocker for browser contexts.

`fixtures/env.ts` exports shared URL and credential constants read from env vars.

`fixtures/test-activity.gpx` is the shared GPX upload fixture.

## Spec Inventory

| Spec | Live server? | What it covers |
|---|---:|---|
| `api.spec.ts` | Yes | Direct HTTP API and ActivityPub endpoint smoke tests. |
| `federation-club.spec.ts` | Yes, + club server | Federated club membership via remote Group actor. |
| `mobile-ux.spec.ts` | Yes | Mobile-viewport (390×844) coverage: bottom nav, sticky header, home feed, settings, post interactions, profile edit, connections modal, People/Clubs pages, and auth forms. |
| `navigation.spec.ts` | Mixed | Auth routing and sidebar navigation. |

## Per-Test Timeout Pattern

The default Playwright test timeout (30 s) is too tight when tests must:
navigate, and wait for WASM hydration (up to 30 s alone).

Specs that include those steps set `test.setTimeout` to 90 s.

| Spec | Timeout |
|---|---|
| `mobile-ux.spec.ts` | 90 s — injectAuth + WASM hydration per test |

When adding new tests that do `injectAuth` + WASM hydration, set the timeout
to at least 90 s for that test or describe block.

## Mobile UX Tests (`mobile-ux.spec.ts`)

Mobile-viewport integration test covering all major UI touchpoints at 390×844
(iPhone 14). Reference screenshots are saved to `screenshots/mobile-ux/` for
QA review.

- **Mobile nav — unauthenticated**: bottom nav visible (Feed, People, Clubs),
  sidebar CSS-hidden; Sign in link in sticky mobile header, not in bottom nav;
  auth-only items absent; header Sign in taps to `/login`; no horizontal overflow.
- **Mobile nav — authenticated**: bottom nav shows Feed/People/Clubs; mobile
  header shows bell button and avatar chip; Sign in absent.
- **Mobile home — guest**: feed container visible; compose card absent; login
  gate visible with Sign in and Create account links; no overflow.
- **Mobile home — authenticated**: compose card and file drop zone visible; no overflow.
- **Mobile settings**: all four sections (Appearance, Privacy, Integrations,
  Danger zone) visible; no overflow; theme toggle flips state; privacy toggle
  flips state; Delete account opens danger-confirm prompt with handle.
- **Mobile post interactions**: like/unlike optimistic update; tapping a feed
  card navigates to exercise detail; thread divider visible; reply composer
  focusable; owner `⋯` menu opens and shows Edit/Delete; no overflow.
- **Mobile profile**: profile card and `@user@domain` handle visible; Edit
  profile button present for owner; edit form opens, saves name, and closes;
  Cancel discards changes; connections modal opens and closes.
- **Mobile auth pages**: login and register forms show all fields; no overflow.
- **Mobile People and Clubs pages**: both pages render without horizontal overflow.
