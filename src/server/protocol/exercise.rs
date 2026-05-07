use activitypub_federation::fetch::object_id::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::server::impls::{actor::DbActor, exercise::DbExercise};
use crate::server::protocol::note::ImageAttachment;

/// `"type": "Exercise"` — fedisport custom vocabulary.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum ExerciseType {
    Exercise,
}

/// ActivityPub wire representation of a fedisport Exercise object.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exercise {
    #[serde(rename = "type")]
    pub kind: ExerciseType,
    pub id: ObjectId<DbExercise>,
    pub attributed_to: ObjectId<DbActor>,
    pub activity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats_url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub to: Vec<Url>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<Url>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachment: Vec<ImageAttachment>,
}
