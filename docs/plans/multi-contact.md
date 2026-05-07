# Multi-Contact Support

## Problem

`local_accounts` stores a single `email` + `phone` (each `UNIQUE`). Login, password reset, and OTP delivery read these directly. Goal: multiple emails and phones per account, any verified contact usable for login or OTP delivery.

Bundled SQLite is 3.47+ so `DROP COLUMN` is safe.

---

## Migration 0002

**New file:** `migrations/0002_account_contacts.sql`

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE account_contacts (
    id          TEXT PRIMARY KEY NOT NULL,
    account_id  TEXT NOT NULL REFERENCES local_accounts(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('email', 'phone')),
    value       TEXT NOT NULL,
    verified    INTEGER NOT NULL DEFAULT 0,
    is_primary  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (value)   -- global: one value cannot belong to two accounts
);

CREATE INDEX account_contacts_value_idx
    ON account_contacts (value) WHERE verified = 1;
CREATE INDEX account_contacts_account_id_idx
    ON account_contacts (account_id);
-- at-most-one primary per (account, kind)
CREATE UNIQUE INDEX ac_primary_idx
    ON account_contacts (account_id, kind) WHERE is_primary = 1;

-- Backfill from existing columns
INSERT INTO account_contacts (id, account_id, kind, value, verified, is_primary)
SELECT lower(hex(randomblob(16))), id, 'email', email, email_verified, 1
FROM local_accounts WHERE email IS NOT NULL;

INSERT INTO account_contacts (id, account_id, kind, value, verified, is_primary)
SELECT lower(hex(randomblob(16))), id, 'phone', phone, phone_verified, 1
FROM local_accounts WHERE phone IS NOT NULL;

ALTER TABLE local_accounts DROP COLUMN email;
ALTER TABLE local_accounts DROP COLUMN phone;
ALTER TABLE local_accounts DROP COLUMN email_verified;
ALTER TABLE local_accounts DROP COLUMN phone_verified;
```

**Note on `otp_requests.purpose` CHECK:** Adding `'contact_verification'` requires recreating the table. Instead, reuse `'registration'` purpose for add-contact OTPs, differentiated by `username = NULL` in the OTP row + presence of an authenticated session.

---

## Rust Models

**`src/db/models/account.rs`**

Remove `email`, `phone`, `email_verified`, `phone_verified` from `LocalAccount`. Add:

```rust
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct AccountContact {
    pub id: String,        // 32-char hex (NOT uuid::Uuid — backfill uses randomblob)
    pub account_id: Uuid,
    pub kind: String,      // "email" | "phone"
    pub value: String,
    pub verified: bool,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
}
```

Re-export from `src/db/models/mod.rs`.

---

## Query Layer

### `src/db/queries/account.rs`

- `create()` — remove `email`, `phone`, `email_verified`, `phone_verified` params
- `find_by_login()` — rewrite with `EXISTS` subquery:
  ```sql
  WHERE (a.username = ?1 AND a.is_local = 1)
     OR EXISTS (
         SELECT 1 FROM account_contacts ac
         WHERE ac.account_id = la.id AND ac.value = ?1 AND ac.verified = 1
     )
  ```
- Remove `find_by_email()` and `find_by_phone()` → single `find_by_contact(value: &str)`:
  ```sql
  SELECT la.* FROM local_accounts la
  JOIN account_contacts ac ON ac.account_id = la.id
  WHERE ac.value = ? AND ac.verified = 1
  LIMIT 1
  ```

### New file: `src/db/queries/contact.rs`

```rust
pub struct ContactQueries;
impl ContactQueries {
    // Read
    list_for_account(pool, account_id) -> Vec<AccountContact>
    find_by_value(pool, value) -> AccountContact        // includes unverified (for dupe check)
    count_verified_for_account(pool, account_id) -> i64

    // Write
    insert_contact(pool, id, account_id, kind, value, verified, is_primary) -> AccountContact
    mark_verified(pool, contact_id) -> ()
    set_primary(pool, contact_id, account_id, kind) -> ()  // transaction: clear-then-set
    delete_contact(pool, contact_id, account_id) -> ()     // guards count > 1 before delete
}
```

Register in `src/db/queries/mod.rs`.

---

## Service Layer

**`src/server/service.rs`**

| Function | Change |
|---|---|
| `do_password_reset_init` | `find_by_email`/`find_by_phone` → `find_by_contact` |
| `do_password_reset_verify` | same |
| `do_register_init` | contact-taken check via `ContactQueries::find_by_value` |
| `do_otp_verify` (registration) | remove email/phone tuple; after `AccountQueries::create`, call `ContactQueries::insert_contact(..., verified: true, is_primary: true)` |
| `seed_owner` | same pattern as above |

**New functions:**

```rust
// Add secondary contact — inserts unverified, issues OTP (purpose = 'registration', username = NULL)
pub async fn do_add_contact(state, account_id: Uuid, raw_value: &str)
    -> Result<(Uuid, Option<String>), AppError>

// Verify the pending contact after OTP is confirmed
pub async fn do_verify_added_contact(state, account_id: Uuid, otp_id: Uuid, code: &str)
    -> Result<(), AppError>

// Remove contact — rejected if it's the last verified one
pub async fn do_remove_contact(state, account_id: Uuid, contact_id: &str)
    -> Result<(), AppError>

// Flip primary flag — transactional
pub async fn do_set_primary_contact(state, account_id: Uuid, contact_id: &str)
    -> Result<(), AppError>
```

---

## API + Frontend

### `src/server/routes/api.rs`

`GET /api/me` — load contacts via `ContactQueries::list_for_account`, include `contacts` array. Keep `email`/`phone` keys derived from primary contact for backward compat.

New routes (register in `src/server/app.rs`):
```
POST   /api/v1/accounts/me/contacts              → do_add_contact
POST   /api/v1/accounts/me/contacts/:id/verify   → do_verify_added_contact
POST   /api/v1/accounts/me/contacts/:id/primary  → do_set_primary_contact
DELETE /api/v1/accounts/me/contacts/:id          → do_remove_contact
```

### `src/web/mod.rs`

Add to `MeResult`:
```rust
pub contacts: Vec<ContactInfo>,
#[serde(default)] pub email: Option<String>,  // derived from primary for compat
#[serde(default)] pub phone: Option<String>,

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ContactInfo {
    pub id: String,
    pub kind: String,      // "email" | "phone"
    pub value: String,
    pub verified: bool,
    pub is_primary: bool,
}
```

### `src/web/server_fns.rs`

`get_me()` — load contacts, map to `Vec<ContactInfo>`, populate both `contacts` and legacy `email`/`phone`.

New server fns: `add_contact`, `verify_contact`, `remove_contact`, `set_primary_contact`.

### Settings page

Add `ContactsSection` to settings: list contacts with verified badge, set-primary and remove buttons, "Add contact" form that triggers OTP flow.

---

## Implementation Order

1. Migration 0002 (schema change before Rust compilation)
2. `LocalAccount` model changes
3. `AccountContact` struct
4. `ContactQueries` in new `contact.rs`
5. Update `AccountQueries` (`create`, `find_by_login`, add `find_by_contact`, remove `find_by_email`/`find_by_phone`)
6. Update service layer
7. Update API routes + `MeResult` + server fns
8. Settings UI
9. `cargo sqlx prepare` to regenerate `.sqlx/` offline data

---

## Gotchas

- `AccountContact.id` is `String` not `Uuid` — backfill hex is not UUID4 format
- `UNIQUE (value)` is global: same email/phone can't be on two accounts (correct)
- `is_primary` uniqueness enforced by partial unique index `ac_primary_idx`, NOT a table constraint
- `ContactQueries::find_by_value` does NOT filter on `verified` — prevents re-registration spam
- `do_register_init` contact-taken check must use `find_by_value` (unverified included)
- After migration, run `cargo sqlx prepare` to regenerate `.sqlx/` or compilation fails offline
