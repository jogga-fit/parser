//! Integration tests for the jogga HTTP API.
//!
//! Each test gets an isolated, ephemeral SQLite database via `tempfile::tempdir()`.
//! The router is built with `build_router` and driven by `axum_test::TestServer`.

#![cfg(not(target_arch = "wasm32"))]

use axum_test::TestServer;
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

use jogga::server::{
    app::build_router,
    cli::{SeedOwnerArgs, seed_owner},
    config::AppConfig,
    state::AppState,
};

// ── Test helpers ────────────────────────────────────────────────────────────

const DOMAIN: &str = "localhost:0";
const OWNER_USERNAME: &str = "owner";
const OWNER_PASSWORD: &str = "hunter22hunter22";
const OWNER_EMAIL: &str = "test@example.com";

/// Build a fresh test server backed by an isolated SQLite file.
/// Returns `(TempDir, TestServer)` — keep `TempDir` alive for the test duration.
async fn make_server() -> (TempDir, TestServer) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("jogga_test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let config = AppConfig::for_test(&db_url, DOMAIN);
    let state = AppState::new(&config).await.expect("AppState::new");

    seed_owner(
        &state.db,
        SeedOwnerArgs {
            username: OWNER_USERNAME.into(),
            email: OWNER_EMAIL.into(),
            password: OWNER_PASSWORD.into(),
            domain: DOMAIN.into(),
            scheme: config.instance.scheme().to_owned(),
        },
    )
    .await
    .expect("seed_owner");

    let (router, _fed_cfg) = build_router(state).await.expect("build_router");
    let server = TestServer::new(router);
    (dir, server)
}

/// Convenience: log in and return the bearer token string.
async fn login(server: &TestServer, login: &str, password: &str) -> String {
    let resp = server
        .post("/api/v1/accounts/token")
        .json(&json!({ "login": login, "password": password }))
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    body["token"].as_str().expect("token field").to_owned()
}

// ── POST /api/v1/accounts/token ─────────────────────────────────────────────

#[tokio::test]
async fn token_valid_credentials_returns_200_and_token() {
    let (_dir, server) = make_server().await;

    let resp = server
        .post("/api/v1/accounts/token")
        .json(&json!({
            "login":    OWNER_EMAIL,
            "password": OWNER_PASSWORD,
        }))
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(
        body["token"].is_string(),
        "expected token field in response"
    );
    assert_eq!(body["username"].as_str(), Some(OWNER_USERNAME));
}

#[tokio::test]
async fn token_wrong_password_returns_401() {
    let (_dir, server) = make_server().await;

    let resp = server
        .post("/api/v1/accounts/token")
        .json(&json!({
            "login":    OWNER_EMAIL,
            "password": "wrongpassword!!",
        }))
        .await;

    resp.assert_status_unauthorized();
}

#[tokio::test]
async fn token_unknown_user_returns_401() {
    let (_dir, server) = make_server().await;

    let resp = server
        .post("/api/v1/accounts/token")
        .json(&json!({
            "login":    "nobody@example.com",
            "password": OWNER_PASSWORD,
        }))
        .await;

    // Unknown account returns 401 (same as wrong password) to prevent account enumeration.
    resp.assert_status_unauthorized();
}

// ── GET /api/v1/accounts/me ─────────────────────────────────────────────────

#[tokio::test]
async fn get_me_with_valid_token_returns_200() {
    let (_dir, server) = make_server().await;
    let token = login(&server, OWNER_EMAIL, OWNER_PASSWORD).await;

    let resp = server
        .get("/api/v1/accounts/me")
        .authorization_bearer(token)
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["username"].as_str(), Some(OWNER_USERNAME));
    assert_eq!(body["email"].as_str(), Some(OWNER_EMAIL));
}

#[tokio::test]
async fn get_me_without_token_returns_401() {
    let (_dir, server) = make_server().await;

    let resp = server.get("/api/v1/accounts/me").await;
    resp.assert_status_unauthorized();
}

#[tokio::test]
async fn get_me_with_invalid_token_returns_401() {
    let (_dir, server) = make_server().await;

    let resp = server
        .get("/api/v1/accounts/me")
        .authorization_bearer("this_is_not_a_real_token_at_all")
        .await;

    resp.assert_status_unauthorized();
}

// ── GET /api/v1/accounts/:username ──────────────────────────────────────────

#[tokio::test]
async fn get_account_existing_local_actor_returns_200() {
    let (_dir, server) = make_server().await;

    let resp = server
        .get(&format!("/api/v1/accounts/{OWNER_USERNAME}"))
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["username"].as_str(), Some(OWNER_USERNAME));
    assert!(body["ap_id"].is_string(), "expected ap_id field");
}

#[tokio::test]
async fn get_account_unknown_username_returns_404() {
    let (_dir, server) = make_server().await;

    let resp = server.get("/api/v1/accounts/nobody_exists_here").await;
    resp.assert_status_not_found();
}

// ── PATCH /api/v1/accounts/me ────────────────────────────────────────────────

#[tokio::test]
async fn update_me_with_valid_token_returns_204() {
    let (_dir, server) = make_server().await;
    let token = login(&server, OWNER_EMAIL, OWNER_PASSWORD).await;

    let resp = server
        .patch("/api/v1/accounts/me")
        .authorization_bearer(&token)
        .json(&json!({
            "display_name": "Jogga Tester",
            "bio": "I run, therefore I am.",
        }))
        .await;

    resp.assert_status_no_content();
}

#[tokio::test]
async fn update_me_display_name_persisted() {
    let (_dir, server) = make_server().await;
    let token = login(&server, OWNER_EMAIL, OWNER_PASSWORD).await;

    server
        .patch("/api/v1/accounts/me")
        .authorization_bearer(&token)
        .json(&json!({ "display_name": "Speed Runner" }))
        .await
        .assert_status_no_content();

    let resp = server
        .get("/api/v1/accounts/me")
        .authorization_bearer(&token)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["display_name"].as_str(), Some("Speed Runner"));
}

#[tokio::test]
async fn update_me_without_token_returns_401() {
    let (_dir, server) = make_server().await;

    let resp = server
        .patch("/api/v1/accounts/me")
        .json(&json!({ "display_name": "Hacker" }))
        .await;

    resp.assert_status_unauthorized();
}

// ── POST /api/v1/accounts/privacy ───────────────────────────────────────────

#[tokio::test]
async fn update_privacy_returns_204() {
    let (_dir, server) = make_server().await;
    let token = login(&server, OWNER_EMAIL, OWNER_PASSWORD).await;

    let resp = server
        .post("/api/v1/accounts/privacy")
        .authorization_bearer(&token)
        .json(&json!({ "public_profile": false }))
        .await;

    resp.assert_status_no_content();
}

#[tokio::test]
async fn update_privacy_toggle_back_to_public_returns_204() {
    let (_dir, server) = make_server().await;
    let token = login(&server, OWNER_EMAIL, OWNER_PASSWORD).await;

    server
        .post("/api/v1/accounts/privacy")
        .authorization_bearer(&token)
        .json(&json!({ "public_profile": false }))
        .await
        .assert_status_no_content();

    server
        .post("/api/v1/accounts/privacy")
        .authorization_bearer(&token)
        .json(&json!({ "public_profile": true }))
        .await
        .assert_status_no_content();
}

#[tokio::test]
async fn update_privacy_without_token_returns_401() {
    let (_dir, server) = make_server().await;

    let resp = server
        .post("/api/v1/accounts/privacy")
        .json(&json!({ "public_profile": false }))
        .await;

    resp.assert_status_unauthorized();
}

// ── POST /api/v1/accounts/password-reset/init ────────────────────────────────

#[tokio::test]
async fn password_reset_init_valid_email_returns_202_and_otp_id() {
    let (_dir, server) = make_server().await;

    let resp = server
        .post("/api/v1/accounts/password-reset/init")
        .json(&json!({ "contact": OWNER_EMAIL }))
        .await;

    resp.assert_status(axum::http::StatusCode::ACCEPTED);
    let body: Value = resp.json();
    assert!(body["otp_id"].is_string(), "expected otp_id field");

    // In debug builds the OTP code is also echoed back.
    #[cfg(debug_assertions)]
    assert!(body["code"].is_string(), "expected code field in debug build");
}

#[tokio::test]
async fn password_reset_init_unknown_email_still_returns_202() {
    let (_dir, server) = make_server().await;

    let resp = server
        .post("/api/v1/accounts/password-reset/init")
        .json(&json!({ "contact": "nobody@example.com" }))
        .await;

    // Always 202 regardless of whether the contact is registered — prevents enumeration.
    resp.assert_status(axum::http::StatusCode::ACCEPTED);
    let body: Value = resp.json();
    assert!(body["otp_id"].is_string(), "expected otp_id field even for unknown contact");
}

#[tokio::test]
async fn password_reset_init_omitting_contact_uses_owner_config_email() {
    let (_dir, server) = make_server().await;

    // When `contact` is absent, the handler uses `config.owner.contact`.
    // AppConfig::for_test sets owner.contact = "test@example.com" (== OWNER_EMAIL),
    // so the owner account is found and a 202 is returned.
    let resp = server
        .post("/api/v1/accounts/password-reset/init")
        .json(&json!({}))
        .await;

    resp.assert_status(axum::http::StatusCode::ACCEPTED);
}

// ── POST /api/v1/accounts/password-reset/verify ─────────────────────────────

/// Full happy path: init → grab code from debug response → verify → new token
///
/// In all builds: verifies init returns 202 with `otp_id`.
/// In debug builds only: verifies with the OTP code and checks the returned token.
///
/// Note: the `code` field is only echoed back in debug builds (the server gates it
/// on `cfg!(debug_assertions)`). In release builds there is no way to retrieve the
/// OTP code from the API response, so the verify flow cannot be tested without a
/// real OTP delivery channel (email/SMS). This is intentional — it prevents
/// test-only backdoors from leaking into production binaries.
#[tokio::test]
async fn password_reset_verify_valid_otp_returns_token() {
    let (_dir, server) = make_server().await;

    // Step 1 — init: always runs in all build profiles.
    let init_resp = server
        .post("/api/v1/accounts/password-reset/init")
        .json(&json!({ "contact": OWNER_EMAIL }))
        .await;
    init_resp.assert_status(axum::http::StatusCode::ACCEPTED);
    let init_body: Value = init_resp.json();

    // Always validate the 202 response structure regardless of build profile.
    assert!(init_body["otp_id"].is_string(), "expected otp_id field in init response");
    let otp_id = init_body["otp_id"].as_str().expect("otp_id");

    // Step 2 — verify: only runs in debug builds where the server echoes the code.
    // In release builds the `code` field is absent; see note in doc comment above.
    #[cfg(debug_assertions)]
    {
        let code = init_body["code"].as_str().expect("code in debug build");

        let verify_resp = server
            .post("/api/v1/accounts/password-reset/verify")
            .json(&json!({
                "otp_id":       otp_id,
                "code":         code,
                "new_password": "newpassword42!",
            }))
            .await;

        verify_resp.assert_status_ok();
        let verify_body: Value = verify_resp.json();
        assert!(
            verify_body["token"].is_string(),
            "expected token field after password reset"
        );

        // Step 3 — verify new token works
        let new_token = verify_body["token"].as_str().unwrap();
        server
            .get("/api/v1/accounts/me")
            .authorization_bearer(new_token)
            .await
            .assert_status_ok();
    }

    // Suppress unused variable warning in release builds.
    let _ = otp_id;
}

#[tokio::test]
async fn password_reset_verify_wrong_code_returns_400() {
    let (_dir, server) = make_server().await;

    let init_resp = server
        .post("/api/v1/accounts/password-reset/init")
        .json(&json!({ "contact": OWNER_EMAIL }))
        .await;
    let otp_id: Value = init_resp.json();
    let otp_id = otp_id["otp_id"].as_str().expect("otp_id");

    let resp = server
        .post("/api/v1/accounts/password-reset/verify")
        .json(&json!({
            "otp_id":       otp_id,
            "code":         "000000",   // very unlikely to be the real code
            "new_password": "newpassword42!",
        }))
        .await;

    resp.assert_status_bad_request();
}

#[tokio::test]
async fn password_reset_verify_invalid_otp_id_returns_400() {
    let (_dir, server) = make_server().await;

    let resp = server
        .post("/api/v1/accounts/password-reset/verify")
        .json(&json!({
            "otp_id":       Uuid::new_v4().to_string(),
            "code":         "123456",
            "new_password": "newpassword42!",
        }))
        .await;

    resp.assert_status_bad_request();
}
