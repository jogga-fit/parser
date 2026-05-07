pub mod accept;
pub mod announce;
pub mod context;
pub mod create;
pub mod create_exercise;
pub mod delete;
pub mod exercise;
pub mod follow;
pub mod group;
pub mod like;
pub mod move_activity;
pub mod note;
pub mod person;
pub mod reject;
pub mod undo;
pub mod undo_like;
pub mod unknown;
pub mod update;

// Imports needed by the enum_delegate-generated Activity impl.
use activitypub_federation::config::Data;
use url::Url;

/// All activity types that the jogga inbox accepts.
///
/// Dispatch order matters for `#[serde(untagged)]`: `CreateExercise` appears
/// before `Create` so that a `Create` activity with `object.type: "Exercise"`
/// is tried first.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
#[enum_delegate::implement(activitypub_federation::traits::Activity)]
pub enum UserAcceptedActivities {
    Follow(follow::Follow),
    Accept(accept::Accept),
    Reject(reject::Reject),
    CreateExercise(create_exercise::CreateExercise),
    Create(create::Create),
    Undo(undo::Undo),
    // UndoLike must come after Undo(Follow).
    UndoLike(undo_like::UndoLike),
    Delete(delete::Delete),
    Like(like::Like),
    Announce(announce::Announce),
    Update(update::Update),
    Move(move_activity::Move),
    /// Open-world assumption: accept any activity type we don't explicitly handle.
    Unknown(unknown::Unknown),
}
