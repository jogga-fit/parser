use activitypub_federation::{
    fetch::object_id::ObjectId, kinds::actor::PersonType, protocol::public_key::PublicKey,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::server::impls::actor::DbActor;

/// ActivityPub `Person` wire type.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    #[serde(rename = "type")]
    pub kind: PersonType,
    pub id: ObjectId<DbActor>,
    #[serde(rename = "preferredUsername")]
    pub preferred_username: String,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub inbox: Url,
    pub outbox: Url,
    pub followers: Url,
    pub following: Url,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<Endpoints>,
    pub public_key: PublicKey,
    pub manually_approves_followers: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_known_as: Vec<Url>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_to: Option<Url>,
}

/// Flexible wire type that accepts both `Person` and `Group` remote actors.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteActor {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: Url,
    #[serde(rename = "preferredUsername")]
    pub preferred_username: String,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub inbox: Url,
    pub outbox: Url,
    pub followers: Url,
    #[serde(default)]
    pub following: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<Endpoints>,
    pub public_key: PublicKey,
    pub manually_approves_followers: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_known_as: Vec<Url>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_to: Option<Url>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_inbox: Option<Url>,
}
