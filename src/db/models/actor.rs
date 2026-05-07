use chrono::{DateTime, Utc};
use sqlx::{
    Decode, Encode, Sqlite, Type,
    encode::IsNull,
    sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef},
};
use url::Url;
use uuid::Uuid;

use crate::db::types::JsonVec;

/// A custom URL wrapper so we can store AP IDs as `url::Url` but serialise as TEXT.
#[derive(Debug, Clone)]
pub struct ApId(pub Url);

impl std::fmt::Display for ApId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Deref for ApId {
    type Target = Url;
    fn deref(&self) -> &Url {
        &self.0
    }
}

impl Type<Sqlite> for ApId {
    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Sqlite> for ApId {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <String as Encode<Sqlite>>::encode(self.0.to_string(), buf)
    }
}

impl<'r> Decode<'r, Sqlite> for ApId {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s = <&str as Decode<Sqlite>>::decode(value)?;
        let url = s.parse::<Url>()?;
        Ok(ApId(url))
    }
}

impl AsRef<str> for ApId {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

/// DB row mapping to the `actors` table.
#[derive(Debug, Clone)]
pub struct ActorRow {
    pub id: Uuid,
    pub ap_id: ApId,
    pub username: String,
    pub domain: String,
    pub actor_type: String,
    pub display_name: Option<String>,
    pub summary: Option<String>,
    pub public_key_pem: String,
    pub private_key_pem: Option<String>,
    pub inbox_url: String,
    pub outbox_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub shared_inbox_url: Option<String>,
    pub manually_approves_followers: bool,
    pub is_local: bool,
    pub is_suspended: bool,
    pub avatar_url: Option<String>,
    pub also_known_as: JsonVec<String>,
    pub moved_to: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
