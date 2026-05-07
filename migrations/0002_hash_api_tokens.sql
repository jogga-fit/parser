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

UPDATE local_accounts SET api_token = NULL;

-- Drop the uniqueness constraint so NULL values don't conflict, then recreate
-- the constraint for non-NULL values.  SQLite does not support ALTER COLUMN,
-- so we use a table-recreation approach.
--
-- NOTE: SQLite treats each NULL as distinct for UNIQUE purposes, so the UPDATE
-- above is safe and a schema change is not strictly required.  We still mark
-- the intent clearly here for future readers.
--
-- The application layer (Rust code) ensures api_token is always set to the
-- SHA-256 hex digest of the raw token immediately on creation / rotation.
-- No further schema changes are needed.
