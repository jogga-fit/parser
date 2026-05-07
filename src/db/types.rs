//! SQLite column wrappers used by query macros.
//!
//! SQLite has no native array or JSON type — we store JSON as `TEXT` and
//! decode/encode it through `serde_json`.  These newtypes plug into sqlx so
//! that `JsonVec<String>` and `JsonValue` can be bound directly inside
//! `query!`/`query_as!` invocations and round-trip cleanly.
//!
//! `DbUrl` is a thin alias — URLs in the multi-user PG schema were also
//! plain `TEXT`, so we expose `String` here for symmetry with the original
//! crate's API surface.

use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use sqlx::Sqlite;
use sqlx::encode::IsNull;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

/// URLs are stored as TEXT in SQLite; expose the type alias so callers don't
/// have to know about the storage representation.
pub type DbUrl = String;

//
// Used for columns like `actors.also_known_as` and `exercises.hidden_stats`
// where the multi-user PG schema used `TEXT[]`.  Default is `'[]'` (empty
// array).  The serde transparent attribute keeps wire-format identical to a
// plain `Vec<T>`.

/// Newtype around `Vec<T>` that round-trips through SQLite as a JSON-encoded
/// `TEXT` column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonVec<T>(pub Vec<T>);

impl<T> JsonVec<T> {
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub fn into_inner(self) -> Vec<T> {
        self.0
    }
}

impl<T> Default for JsonVec<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> Deref for JsonVec<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T> DerefMut for JsonVec<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}

impl<T> From<Vec<T>> for JsonVec<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
    }
}

impl<T> From<JsonVec<T>> for Vec<T> {
    fn from(v: JsonVec<T>) -> Vec<T> {
        v.0
    }
}

impl<T> sqlx::Type<Sqlite> for JsonVec<T> {
    fn type_info() -> SqliteTypeInfo {
        <String as sqlx::Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as sqlx::Type<Sqlite>>::compatible(ty)
    }
}

impl<'q, T> sqlx::Encode<'q, Sqlite> for JsonVec<T>
where
    T: Serialize,
{
    fn encode_by_ref(
        &self,
        buf: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = serde_json::to_string(&self.0)?;
        <String as sqlx::Encode<Sqlite>>::encode(s, buf)
    }
}

impl<'r, T> sqlx::Decode<'r, Sqlite> for JsonVec<T>
where
    T: for<'de> Deserialize<'de>,
{
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s = <&str as sqlx::Decode<Sqlite>>::decode(value)?;
        let v: Vec<T> = serde_json::from_str(s)?;
        Ok(JsonVec(v))
    }
}

//
// Wraps `serde_json::Value` for TEXT-stored JSON columns (`ap_json`,
// `content_map`, `route`).  sqlx 0.8 already supports JSON via the `Json<T>`
// adapter, but defining our own makes the binding sites explicit and avoids
// confusing macro inference when the column holds NULL.

/// Newtype around `serde_json::Value` for SQLite JSON-as-TEXT columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonValue(pub serde_json::Value);

impl JsonValue {
    #[must_use]
    pub fn new(v: serde_json::Value) -> Self {
        Self(v)
    }

    #[must_use]
    pub fn into_inner(self) -> serde_json::Value {
        self.0
    }
}

impl Deref for JsonValue {
    type Target = serde_json::Value;
    fn deref(&self) -> &serde_json::Value {
        &self.0
    }
}

impl DerefMut for JsonValue {
    fn deref_mut(&mut self) -> &mut serde_json::Value {
        &mut self.0
    }
}

impl From<serde_json::Value> for JsonValue {
    fn from(v: serde_json::Value) -> Self {
        Self(v)
    }
}

impl From<JsonValue> for serde_json::Value {
    fn from(v: JsonValue) -> serde_json::Value {
        v.0
    }
}

impl sqlx::Type<Sqlite> for JsonValue {
    fn type_info() -> SqliteTypeInfo {
        <String as sqlx::Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as sqlx::Type<Sqlite>>::compatible(ty)
    }
}

impl<'q> sqlx::Encode<'q, Sqlite> for JsonValue {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = serde_json::to_string(&self.0)?;
        <String as sqlx::Encode<Sqlite>>::encode(s, buf)
    }
}

impl<'r> sqlx::Decode<'r, Sqlite> for JsonValue {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s = <&str as sqlx::Decode<Sqlite>>::decode(value)?;
        let v: serde_json::Value = serde_json::from_str(s)?;
        Ok(JsonValue(v))
    }
}
