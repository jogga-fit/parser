use activitypub_federation::{
    config::Data, fetch::object_id::ObjectId, kinds::object::NoteType,
    protocol::verification::verify_domains_match, traits::Object,
};
use async_trait::async_trait;
use url::Url;

use crate::db::{
    models::ObjectRow,
    queries::{ObjectQueries, object::NewObject},
};
use crate::server::{
    error::{AppError, InternalError},
    protocol::note::{Note, visibility_from_to_cc},
    state::AppState,
};

/// Wrapper around `ObjectRow` for trait impls.
///
/// Unlike fedisport (where `ObjectRow.ap_id` is an `ApId` newtype holding a `Url`),
/// jogga stores `ap_id` as `String`. We eagerly parse it so `Object::id()` can
/// return a `&Url`.
#[derive(Debug, Clone)]
pub struct DbNote {
    pub row: ObjectRow,
    pub ap_id: Url,
}

impl DbNote {
    /// Construct from `ObjectRow`, parsing `ap_id`.
    pub fn from_row(row: ObjectRow) -> Result<Self, AppError> {
        let ap_id = row.ap_id.parse().map_err(AppError::from)?;
        Ok(Self { row, ap_id })
    }
}

#[async_trait]
impl Object for DbNote {
    type DataType = AppState;
    type Kind = Note;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.ap_id
    }

    async fn read_from_id(object_id: Url, data: &Data<AppState>) -> Result<Option<Self>, AppError> {
        match ObjectQueries::find_by_ap_id(&data.db, object_id.as_str()).await {
            Ok(row) => Ok(Some(DbNote {
                ap_id: object_id,
                row,
            })),
            Err(crate::db::DbError::NotFound) => Ok(None),
            Err(e) => Err(AppError::from(e)),
        }
    }

    async fn into_json(self, data: &Data<AppState>) -> Result<Note, AppError> {
        let row = &self.row;
        let scheme = data.app_data().config.instance.scheme();
        let domain = data.domain();
        let note_uuid = self
            .ap_id
            .path_segments()
            .and_then(|mut s| s.next_back())
            .unwrap_or("")
            .to_owned();
        let replies_id = format!("{scheme}://{domain}/notes/{note_uuid}/replies");
        let replies = serde_json::json!({
            "type": "Collection",
            "id": replies_id,
            "totalItems": row.reply_count,
        });
        Ok(Note {
            kind: NoteType::Note,
            id: ObjectId::from(self.ap_id.clone()),
            attributed_to: ObjectId::from(
                row.attributed_to.parse::<Url>().map_err(AppError::from)?,
            ),
            content: row.content.clone().unwrap_or_default(),
            summary: row.summary.clone(),
            sensitive: row.sensitive,
            in_reply_to: row.in_reply_to.as_ref().and_then(|u| u.parse().ok()),
            url: row.url.as_ref().and_then(|u| u.parse().ok()),
            to: vec![],
            cc: vec![],
            attachment: vec![],
            replies: Some(replies),
        })
    }

    async fn verify(
        json: &Note,
        expected_domain: &Url,
        _data: &Data<AppState>,
    ) -> Result<(), AppError> {
        verify_domains_match(json.id.inner(), expected_domain)
            .map_err(|e| AppError::Internal(InternalError::Federation(e.to_string())))?;
        Ok(())
    }

    async fn from_json(json: Note, data: &Data<AppState>) -> Result<Self, AppError> {
        let vis = visibility_from_to_cc(&json.to, &json.cc);
        let ap_id = json.id.inner().clone();
        let new_obj = NewObject {
            ap_id: ap_id.as_str(),
            object_type: "Note",
            attributed_to: json.attributed_to.inner().as_str(),
            actor_id: None,
            content: Some(json.content.as_str()),
            content_map: None,
            summary: json.summary.as_deref(),
            sensitive: json.sensitive,
            in_reply_to: json.in_reply_to.as_ref().map(|u| u.as_str()),
            published: None,
            url: json.url.as_ref().map(|u| u.as_str()),
            ap_json: serde_json::to_value(&json).unwrap_or(serde_json::Value::Null),
            visibility: vis,
        };
        let row = ObjectQueries::insert(&data.db, &new_obj).await?;
        Ok(DbNote { ap_id, row })
    }
}
