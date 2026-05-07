//! Auth / account management service functions.

use chrono::Utc;
use rsa::{
    RsaPrivateKey, RsaPublicKey,
    pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding},
};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::queries::{
    AccountQueries, ActorQueries, OtpQueries, actor::NewActor,
};
use crate::server::{
    auth::{
        generate_otp, generate_token, hash_otp, hash_password, validate_email, validate_password,
        validate_phone, validate_username, verify_otp,
    },
    error::{AppError, CryptoError, InternalError},
    notify::ContactType,
    state::AppState,
};

#[tracing::instrument(skip(state, password))]
pub async fn do_login(state: &AppState, login: &str, password: &str) -> Result<String, AppError> {
    let account = AccountQueries::find_by_login(&state.db, login)
        .await
        .map_err(|_| {
            debug!(login, "login attempt — account not found");
            AppError::Unauthorized
        })?;

    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| AppError::Unauthorized)?;

    if actor.is_suspended {
        warn!(
            username = actor.username,
            "login attempt on suspended account"
        );
        return Err(AppError::Unauthorized);
    }

    if !crate::server::auth::verify_password(password, &account.password_hash)? {
        warn!(username = actor.username, "login attempt — wrong password");
        return Err(AppError::Unauthorized);
    }

    info!(username = actor.username, "login successful");
    Ok(account.api_token)
}

/// Issue a password-reset OTP. For jogga: uses the owner contact from config.
/// Returns `(otp_id, dev_code)` — `dev_code` is `Some` only in debug builds.
#[tracing::instrument(skip(state, contact))]
pub async fn do_password_reset_init(
    state: &AppState,
    contact: &str,
) -> Result<(Uuid, Option<String>), AppError> {
    let (normalised, contact_type_str, contact_type) = if contact.contains('@') {
        crate::server::auth::validate_email(contact)?;
        (contact.to_lowercase(), "email", ContactType::Email)
    } else {
        crate::server::auth::validate_phone(contact)?;
        (contact.to_string(), "phone", ContactType::Phone)
    };

    // Silently ignore whether account exists — always return 202 to prevent contact enumeration.
    let account_found = match contact_type {
        ContactType::Email => AccountQueries::find_by_email(&state.db, &normalised).await.is_ok(),
        ContactType::Phone => AccountQueries::find_by_phone(&state.db, &normalised).await.is_ok(),
    };

    if !account_found {
        debug!(contact_type = contact_type_str, "password-reset requested for unknown contact — returning dummy 202");
        return Ok((Uuid::nil(), None));
    }

    let code = generate_otp();
    let hash = hash_otp(&code)?;
    let expires = Utc::now() + chrono::Duration::minutes(15);

    let otp = OtpQueries::insert(
        &state.db,
        &normalised,
        contact_type_str,
        "password_reset",
        None,
        &hash,
        expires,
    )
    .await?;

    info!(contact_type = contact_type_str, otp_id = %otp.id, "password-reset OTP issued");

    let dev_code = if cfg!(debug_assertions) {
        warn!(otp_id = %otp.id, "debug build — returning OTP in response");
        Some(code)
    } else {
        state
            .notifier
            .send(&normalised, contact_type, "password_reset", &code)
            .await
            .inspect_err(|e| warn!(error = %e, "OTP delivery failed"))?;
        None
    };

    Ok((otp.id, dev_code))
}

/// Verify OTP, update password, return fresh token.
#[tracing::instrument(skip(state, code, new_password))]
pub async fn do_password_reset_verify(
    state: &AppState,
    otp_id: Uuid,
    code: &str,
    new_password: &str,
) -> Result<String, AppError> {
    validate_password(new_password)?;

    const MAX_OTP_ATTEMPTS: u8 = 5;

    let otp = OtpQueries::find_active(&state.db, otp_id)
        .await
        .map_err(|_| AppError::BadRequest("Code is invalid, expired, or already used.".into()))?;

    if otp.purpose != "password_reset" {
        return Err(AppError::BadRequest(
            "Code is not valid for password reset.".into(),
        ));
    }

    {
        let attempts = state.otp_attempts.lock().unwrap_or_else(|e| e.into_inner());
        if attempts.get(&otp.id).copied().unwrap_or(0) >= MAX_OTP_ATTEMPTS {
            return Err(AppError::BadRequest(
                "Too many incorrect attempts — request a new code.".into(),
            ));
        }
    }

    if !verify_otp(code, &otp.code_hash)? {
        let mut attempts = state.otp_attempts.lock().unwrap_or_else(|e| e.into_inner());
        let count = attempts.entry(otp.id).or_insert(0);
        *count += 1;
        warn!(otp_id = %otp.id, attempts = *count, "password-reset wrong code");
        return Err(AppError::BadRequest("Incorrect code.".into()));
    }

    state
        .otp_attempts
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&otp.id);
    OtpQueries::mark_used(&state.db, otp.id).await?;

    let account = match otp.contact_type.as_str() {
        "email" => AccountQueries::find_by_email(&state.db, &otp.contact).await,
        "phone" => AccountQueries::find_by_phone(&state.db, &otp.contact).await,
        other => {
            return Err(AppError::Internal(InternalError::DataIntegrity(format!(
                "unknown contact_type '{other}'"
            ))));
        }
    }
    .map_err(|_| AppError::BadRequest("Account not found.".into()))?;

    let new_hash = hash_password(new_password)?;
    let new_token = generate_token();

    AccountQueries::update_password(&state.db, account.id, &new_hash, &new_token).await?;

    info!(account_id = %account.id, "password reset completed");
    Ok(new_token)
}

// ── Registration init ─────────────────────────────────────────────────────────

#[tracing::instrument(skip(state, email, phone))]
pub async fn do_register_init(
    state: &AppState,
    username: &str,
    email: Option<&str>,
    phone: Option<&str>,
) -> Result<(Uuid, Option<String>), AppError> {
    validate_username(username)?;

    let (contact, contact_type_str, contact_type) = match (email, phone) {
        (Some(email), None) => {
            validate_email(email)?;
            (email.to_lowercase(), "email", ContactType::Email)
        }
        (None, Some(phone)) => {
            validate_phone(phone)?;
            (phone.to_string(), "phone", ContactType::Phone)
        }
        _ => {
            return Err(AppError::BadRequest(
                "provide exactly one of 'email' or 'phone'".into(),
            ));
        }
    };

    let username_taken = ActorQueries::find_local_by_username(&state.db, username)
        .await
        .is_ok();
    let contact_taken = match contact_type {
        ContactType::Email => AccountQueries::find_by_email(&state.db, &contact).await.is_ok(),
        ContactType::Phone => AccountQueries::find_by_phone(&state.db, &contact).await.is_ok(),
    };
    if username_taken || contact_taken {
        return Err(AppError::BadRequest(
            "username or contact already in use".into(),
        ));
    }

    let code = generate_otp();
    let hash = hash_otp(&code)?;
    let expires = Utc::now() + chrono::Duration::minutes(15);

    let otp = OtpQueries::insert(
        &state.db,
        &contact,
        contact_type_str,
        "registration",
        Some(username),
        &hash,
        expires,
    )
    .await?;

    info!(
        username,
        contact_type = contact_type_str,
        otp_id = %otp.id,
        "registration OTP issued"
    );

    let dev_code = if cfg!(debug_assertions) {
        warn!(otp_id = %otp.id, "debug build — returning OTP in response");
        Some(code)
    } else {
        state
            .notifier
            .send(&contact, contact_type, "registration", &code)
            .await
            .inspect_err(|e| warn!(error = %e, "OTP delivery failed"))?;
        None
    };

    Ok((otp.id, dev_code))
}

// ── OTP verify (unified) ──────────────────────────────────────────────────────

pub struct OtpVerifyOutcome {
    pub token: String,
    pub username: Option<String>,
    pub ap_id: Option<String>,
}

#[tracing::instrument(skip(state, code, password, display_name))]
pub async fn do_otp_verify(
    state: &AppState,
    otp_id: Uuid,
    code: &str,
    password: &str,
    display_name: Option<&str>,
) -> Result<OtpVerifyOutcome, AppError> {
    validate_password(password)?;

    const MAX_OTP_ATTEMPTS: u8 = 5;

    let otp = OtpQueries::find_active(&state.db, otp_id)
        .await
        .map_err(|_| AppError::BadRequest("Code is invalid, expired, or already used.".into()))?;

    {
        let attempts = state.otp_attempts.lock().unwrap_or_else(|e| e.into_inner());
        if attempts.get(&otp.id).copied().unwrap_or(0) >= MAX_OTP_ATTEMPTS {
            return Err(AppError::BadRequest(
                "Too many incorrect attempts — request a new code.".into(),
            ));
        }
    }

    if !verify_otp(code, &otp.code_hash)? {
        let mut attempts = state.otp_attempts.lock().unwrap_or_else(|e| e.into_inner());
        let count = attempts.entry(otp.id).or_insert(0);
        *count += 1;
        warn!(otp_id = %otp.id, attempts = *count, "OTP verification failed — wrong code");
        return Err(AppError::BadRequest("Incorrect code.".into()));
    }

    state
        .otp_attempts
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&otp.id);

    match otp.purpose.as_str() {
        "registration" => {
            let username = otp.username.as_deref().ok_or_else(|| {
                AppError::Internal(InternalError::DataIntegrity("OTP missing username".into()))
            })?;

            let (contact_email, contact_phone, email_verified, phone_verified) =
                match otp.contact_type.as_str() {
                    "email" => (Some(otp.contact.as_str()), None, true, false),
                    "phone" => (None, Some(otp.contact.as_str()), false, true),
                    other => {
                        return Err(AppError::Internal(InternalError::DataIntegrity(format!(
                            "unknown contact_type '{other}'"
                        ))));
                    }
                };

            let domain = &state.config.instance.domain;
            let scheme = state.config.instance.scheme();
            let base = format!("{scheme}://{domain}/users/{username}");

            if ActorQueries::find_local_by_username(&state.db, username)
                .await
                .is_ok()
            {
                return Err(AppError::BadRequest(format!(
                    "username '{username}' is no longer available"
                )));
            }

            debug!("generating RSA 2048 keypair");
            let (pub_pem, priv_pem) = tokio::task::spawn_blocking(|| -> Result<_, CryptoError> {
                let private_key = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)
                    .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
                let public_key = RsaPublicKey::from(&private_key);
                let pub_pem = public_key
                    .to_public_key_pem(LineEnding::LF)
                    .map_err(|e| CryptoError::PemEncode(e.to_string()))?;
                let priv_pem = private_key
                    .to_pkcs8_pem(LineEnding::LF)
                    .map_err(|e| CryptoError::PemEncode(e.to_string()))?
                    .to_string();
                Ok((pub_pem, priv_pem))
            })
            .await
            .map_err(|e| CryptoError::KeyGen(e.to_string()))??;

            let password_hash = hash_password(password)?;
            let token = generate_token();

            OtpQueries::mark_used(&state.db, otp.id).await?;

            let mut conn = state
                .db
                .acquire()
                .await
                .map_err(|e| AppError::from(crate::db::DbError::Sqlx(e)))?;

            let actor = ActorQueries::insert(
                &mut *conn,
                &NewActor {
                    ap_id: &base,
                    username,
                    domain,
                    actor_type: "Person",
                    display_name,
                    summary: None,
                    public_key_pem: &pub_pem,
                    private_key_pem: Some(&priv_pem),
                    inbox_url: &format!("{base}/inbox"),
                    outbox_url: &format!("{base}/outbox"),
                    followers_url: &format!("{base}/followers"),
                    following_url: &format!("{base}/following"),
                    shared_inbox_url: Some(&format!("{scheme}://{domain}/inbox")),
                    manually_approves_followers: false,
                    is_local: true,
                    ap_json: None,
                    also_known_as: &[],
                    moved_to: None,
                },
            )
            .await?;

            let account_id = Uuid::new_v4();
            AccountQueries::create(
                &state.db,
                account_id,
                actor.id,
                &password_hash,
                &token,
                contact_email,
                contact_phone,
                email_verified,
                phone_verified,
            )
            .await?;

            info!(username, actor_id = %actor.id, "account created");

            Ok(OtpVerifyOutcome {
                token,
                username: Some(actor.username),
                ap_id: Some(actor.ap_id.to_string()),
            })
        }

        "password_reset" => {
            OtpQueries::mark_used(&state.db, otp.id).await?;

            let account = match otp.contact_type.as_str() {
                "email" => AccountQueries::find_by_email(&state.db, &otp.contact).await,
                "phone" => AccountQueries::find_by_phone(&state.db, &otp.contact).await,
                other => {
                    return Err(AppError::Internal(InternalError::DataIntegrity(format!(
                        "unknown contact_type '{other}'"
                    ))));
                }
            }
            .map_err(|_| AppError::BadRequest("Account not found.".into()))?;

            let new_hash = hash_password(password)?;
            let new_token = generate_token();

            AccountQueries::update_password(&state.db, account.id, &new_hash, &new_token).await?;

            info!(account_id = %account.id, "password reset completed via otp_verify");

            Ok(OtpVerifyOutcome {
                token: new_token,
                username: None,
                ap_id: None,
            })
        }

        other => Err(AppError::BadRequest(format!(
            "OTP purpose '{other}' is not handled by this endpoint"
        ))),
    }
}

/// Create the single-owner actor + account. Called from `seed-owner` CLI subcommand.
pub async fn seed_owner(
    pool: &crate::db::SqlitePool,
    username: &str,
    password: &str,
    domain: &str,
    scheme: &str,
    contact: &str,
) -> Result<(), AppError> {
    let base = format!("{scheme}://{domain}/users/{username}");
    let inbox_url = format!("{base}/inbox");
    let outbox_url = format!("{base}/outbox");
    let followers_url = format!("{base}/followers");
    let following_url = format!("{base}/following");

    // Generate RSA 2048 keypair.
    debug!("generating RSA 2048 keypair for owner {username}");
    let (pub_pem, priv_pem) = tokio::task::spawn_blocking(|| -> Result<_, CryptoError> {
        let private_key = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)
            .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
        let public_key = RsaPublicKey::from(&private_key);
        let pub_pem = public_key
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| CryptoError::PemEncode(e.to_string()))?;
        let priv_pem = private_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| CryptoError::PemEncode(e.to_string()))?
            .to_string();
        Ok((pub_pem, priv_pem))
    })
    .await
    .map_err(|e| AppError::Internal(InternalError::Unexpected(e.to_string())))??;

    let new_actor = NewActor {
        ap_id: &base,
        username,
        domain,
        actor_type: "Person",
        display_name: None,
        summary: None,
        public_key_pem: &pub_pem,
        private_key_pem: Some(&priv_pem),
        inbox_url: &inbox_url,
        outbox_url: &outbox_url,
        followers_url: &followers_url,
        following_url: &following_url,
        shared_inbox_url: Some(&format!("{scheme}://{domain}/inbox")),
        manually_approves_followers: false,
        is_local: true,
        ap_json: None,
        also_known_as: &[],
        moved_to: None,
    };

    // Refuse if an owner already exists (one-shot invariant).
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM local_accounts")
        .fetch_one(pool)
        .await
        .map_err(crate::db::DbError::Sqlx)?;
    if existing > 0 {
        return Err(AppError::Conflict("owner already seeded".into()));
    }

    let mut conn = pool.acquire().await.map_err(crate::db::DbError::Sqlx)?;
    let actor_row = ActorQueries::insert(&mut conn, &new_actor).await?;

    let password_hash = hash_password(password)?;
    let api_token = generate_token();
    let account_id = Uuid::new_v4();

    let (email, phone, email_verified, phone_verified) = if contact.contains('@') {
        (Some(contact.to_owned()), None::<String>, true, false)
    } else {
        (None::<String>, Some(contact.to_owned()), false, true)
    };

    crate::db::queries::AccountQueries::create(
        pool,
        account_id,
        actor_row.id,
        &password_hash,
        &api_token,
        email.as_deref(),
        phone.as_deref(),
        email_verified,
        phone_verified,
    )
    .await?;

    info!(username, actor_id = %actor_row.id, "owner seeded");
    Ok(())
}
