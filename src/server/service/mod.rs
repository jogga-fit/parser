//! Business logic shared across route handlers.
//!
//! This module is split into domain-focused submodules:
//! - [`auth`]        — login, registration, OTP, password reset, seed_owner
//! - [`social`]      — follow, unfollow, alias, move, resolve handle
//! - [`exercise`]    — upload exercise, update post, announce
//! - [`interaction`] — like, unlike

mod helpers;

pub mod auth;
pub mod exercise;
pub mod interaction;
pub mod social;
pub mod storage;

// ── Re-exports (preserve the original flat `service::*` public API) ───────────

pub use auth::{
    OtpVerifyOutcome, do_login, do_otp_verify, do_password_reset_init, do_password_reset_verify,
    do_register_init, seed_owner,
};

pub use social::{
    do_add_alias, do_follow, do_move_account, do_remove_alias, do_unfollow, resolve_handle,
};

pub use exercise::{
    ExerciseUploadRequest, ExerciseUploadResult, UpdatePostRequest, VALID_VISIBILITY, do_announce,
    do_update_post, do_upload_exercise,
};

pub use interaction::{do_like, do_unlike};
