use dioxus::prelude::ServerFnError;
use serde::{Deserialize, Serialize};

pub mod app;
pub mod browser;
pub mod components;
pub mod format;
pub mod hooks;
pub mod image;
pub mod pages;
pub mod server_fns;
pub mod state;

/// Cross-platform async sleep.
///
/// Uses [`gloo_timers`] on `wasm32` and [`tokio::time::sleep`] on native targets,
/// so UI components can call it without any `cfg` guards at the call site.
pub async fn sleep_ms(ms: u32) {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::TimeoutFuture::new(ms).await;

    #[cfg(not(target_arch = "wasm32"))]
    tokio::time::sleep(std::time::Duration::from_millis(u64::from(ms))).await;
}

/// Strip Dioxus server-function error boilerplate.
///
/// Dioxus wraps `ServerFnError::new(msg)` as:
/// `"error running server function: {msg} (details: None)"`.
/// This extracts just the human-readable `{msg}`.
pub fn sfn_msg(e: &ServerFnError) -> String {
    let s = e.to_string();
    s.strip_prefix("error running server function: ")
        .and_then(|s| s.strip_suffix(" (details: None)"))
        .unwrap_or(&s)
        .to_string()
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
pub mod server;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginResult {
    pub token: String,
    pub username: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RegisterInitResult {
    pub otp_id: String,
    /// Only set in debug builds; always `None` in release.
    pub code: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RegisterVerifyResult {
    pub username: String,
    pub ap_id: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MeResult {
    pub username: String,
    pub ap_id: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
    pub public_profile: bool,
    pub theme: String,
    #[serde(default)]
    pub also_known_as: Vec<String>,
    #[serde(default)]
    pub moved_to: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DirectoryItem {
    pub username: String,
    pub domain: String,
    pub ap_id: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FollowingItem {
    pub ap_id: String,
    pub username: String,
    pub domain: String,
    pub is_local: bool,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub accepted: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FollowerItem {
    pub ap_id: String,
    pub username: String,
    pub domain: String,
    pub is_local: bool,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub accepted: bool,
    /// Original AP Follow activity id URL — None for pre-migration follows.
    pub follow_ap_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ActorInfo {
    pub username: String,
    pub domain: String,
    pub ap_id: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub public_profile: bool,
    pub followers_count: i64,
    pub following_count: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FeedItem {
    pub id: String,
    /// The AP ID of the object (Note/Exercise) — used for like/unlike actions.
    pub object_ap_id: String,
    pub actor_username: String,
    pub actor_domain: String,
    pub actor_is_local: bool,
    pub actor_ap_id: String,
    pub actor_avatar_url: Option<String>,
    pub activity_type: String,
    /// "Note" | "Exercise"
    pub object_type: String,
    pub content: Option<String>,
    pub published: String,
    /// Exercise sub-type: run, ride, swim, walk, hike.
    pub exercise_type: Option<String>,
    pub duration_s: Option<i64>,
    pub distance_m: Option<f64>,
    pub elevation_gain_m: Option<f64>,
    pub avg_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub avg_power_w: Option<f64>,
    pub max_power_w: Option<f64>,
    pub normalized_power_w: Option<f64>,
    pub avg_cadence_rpm: Option<f64>,
    pub avg_pace_s_per_km: Option<f64>,
    pub device: Option<String>,
    pub title: Option<String>,
    pub image_urls: Vec<String>,
    pub like_count: i64,
    pub viewer_has_liked: bool,
    pub viewer_is_owner: bool,
    pub reply_count: i64,
    /// AP ID of the parent object, if this is a reply.
    pub in_reply_to: Option<String>,
    /// Route GeoJSON endpoint URL — present for Exercise objects that have a recorded route.
    pub route_url: Option<String>,
    /// Stats currently hidden from display (Exercise only). Empty for Notes and
    /// for feed items loaded from activity ap_json (home feed).
    pub hidden_stats: Vec<String>,
    /// Set when this post was shown via a club Announce.
    /// Contains the club's `handle` (username portion, no `@`).
    pub via_club_handle: Option<String>,
    /// Human-readable club display name (falls back to handle).
    pub via_club_display: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NotificationItem {
    pub id: String,
    /// "follow_request" | "new_follower" | "follow_accepted" | "like"
    pub kind: String,
    pub from_ap_id: String,
    pub from_username: String,
    pub from_display_name: Option<String>,
    pub from_avatar_url: Option<String>,
    /// Populated for "like" notifications.
    pub object_ap_id: Option<String>,
    pub object_title: Option<String>,
    pub is_read: bool,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ThreadItem {
    pub ap_id: String,
    pub author_username: String,
    pub author_avatar_url: Option<String>,
    pub content: Option<String>,
    pub published: String,
    pub like_count: i64,
    pub viewer_has_liked: bool,
    pub viewer_is_owner: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct UploadExerciseMeta {
    pub activity_type: String,
    pub visibility: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_urls: Vec<String>,
    pub hidden_stats: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UploadExerciseResult {
    pub id: String,
    pub ap_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConnectionItem {
    pub ap_id: String,
    pub username: String,
    pub domain: String,
    pub is_local: bool,
    pub display_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConnectionsResult {
    /// false when the profile is private and the viewer is not an authorized follower.
    pub visible: bool,
    pub following: Vec<ConnectionItem>,
    pub followers: Vec<ConnectionItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OtpVerifyResult {
    pub token: String,
    /// Set for registration OTPs; `None` for password reset.
    pub username: Option<String>,
    /// Set for registration OTPs; `None` for password reset.
    pub ap_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClubItem {
    pub handle: String,
    pub ap_id: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub exclusive: bool,
    pub member_count: i64,
    /// `None` = not a member, `"member"` = joined, `"moderator"`, `"admin"`.
    pub my_role: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClubMemberItem {
    pub ap_id: String,
    pub username: String,
    pub domain: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    /// `None` = plain member, `"moderator"`, `"admin"`.
    pub role: Option<String>,
    pub accepted: bool,
}
