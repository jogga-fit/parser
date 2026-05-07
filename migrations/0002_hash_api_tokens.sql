-- Migrate local_accounts to store SHA-256 hashes of bearer tokens instead of
-- plaintext tokens (C3: tokens at rest must not be reversible).
--
-- Existing sessions are invalidated: the stored plaintext tokens cannot be
-- reverse-hashed, so all users must re-authenticate after this migration.
-- For a new project with no production data this is fully acceptable.
--
-- The column is kept with the same name (api_token) — the column now stores
-- a 64-character lowercase hex SHA-256 digest instead of the raw token.
-- Existing rows are set to NULL (forces re-auth) because we cannot compute the
-- correct hash without the original random bytes.

-- api_token is NOT NULL + UNIQUE. We can't set it to NULL.
-- Set a per-row placeholder that cannot match any SHA-256 hex digest (which is
-- always exactly 64 lowercase hex characters). This invalidates existing sessions
-- without violating constraints — users must re-authenticate after this migration.
UPDATE local_accounts SET api_token = 'invalidated-' || CAST(rowid AS TEXT);

-- The application layer (Rust) now stores SHA-256(raw_token) in this column
-- and hashes the incoming token before every lookup. No schema change required.
