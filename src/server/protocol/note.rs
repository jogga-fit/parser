use activitypub_federation::{fetch::object_id::ObjectId, kinds::object::NoteType};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::server::impls::{actor::DbActor, note::DbNote};

/// AS2 Image attachment.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImageAttachment {
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub media_type: String,
}

const AS_PUBLIC: &str = "https://www.w3.org/ns/activitystreams#Public";

/// Derive a visibility string from AP `to`/`cc` arrays.
pub fn visibility_from_to_cc(to: &[Url], cc: &[Url]) -> &'static str {
    let to_has_public = to.iter().any(|u| u.as_str() == AS_PUBLIC);
    if to_has_public {
        return "public";
    }
    let cc_has_public = cc.iter().any(|u| u.as_str() == AS_PUBLIC);
    if cc_has_public {
        return "unlisted";
    }
    let to_has_followers = to.iter().any(|u| u.path().ends_with("/followers"));
    if to_has_followers {
        return "followers";
    }
    "private"
}

/// ActivityPub `Note` wire type.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    #[serde(rename = "type")]
    pub kind: NoteType,
    pub id: ObjectId<DbNote>,
    pub attributed_to: ObjectId<DbActor>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub sensitive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Url>,
    #[serde(
        default,
        deserialize_with = "activitypub_federation::protocol::helpers::deserialize_one_or_many"
    )]
    pub to: Vec<Url>,
    #[serde(
        default,
        deserialize_with = "activitypub_federation::protocol::helpers::deserialize_one_or_many"
    )]
    pub cc: Vec<Url>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachment: Vec<ImageAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replies: Option<serde_json::Value>,
}
