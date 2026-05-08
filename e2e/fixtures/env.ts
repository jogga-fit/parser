export const WEB = process.env.BASE_URL || 'http://localhost:6060';
export const OWNER_EMAIL = process.env.OWNER_EMAIL || 'owner@jogga.test';
export const OWNER_PASSWORD = process.env.OWNER_PASSWORD || 'testpass99';
export const SKIP_INTEGRATION = !process.env.INTEGRATION_TEST;

// Remote club server (fedisport-compatible, Group actor federation)
export const CLUB_SERVER_URL = process.env.CLUB_SERVER_URL ?? '';
export const CLUB_SERVER_DOMAIN = process.env.CLUB_SERVER_DOMAIN
  ?? (CLUB_SERVER_URL ? new URL(CLUB_SERVER_URL).host : '');
// Pre-existing admin token — if set, skips user registration on the club server.
// Requires dev.otp_echo = true on the club server when not set.
export const CLUB_ADMIN_TOKEN = process.env.CLUB_ADMIN_TOKEN ?? '';
export const SKIP_CLUB_FED = !process.env.INTEGRATION_TEST || !process.env.CLUB_SERVER_URL;
