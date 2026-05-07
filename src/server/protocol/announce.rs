use activitypub_federation::{
    config::Data, kinds::activity::AnnounceType, protocol::verification::verify_domains_match,
    traits::Activity,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::warn;
use url::Url;
use uuid::Uuid;

use crate::server::{
    error::{AppError, InternalError},
    state::AppState,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Announce {
    #[serde(rename = "type")]
    pub kind: AnnounceType,
    pub id: Url,
    pub actor: Url,
    pub object: Url,
}

#[async_trait]
impl Activity for Announce {
    type DataType = AppState;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.id
    }
    fn actor(&self) -> &Url {
        &self.actor
    }

    async fn verify(&self, _data: &Data<AppState>) -> Result<(), AppError> {
        verify_domains_match(&self.actor, &self.id)
            .map_err(|e| AppError::Internal(InternalError::Federation(e.to_string())))?;
        Ok(())
    }

    async fn receive(self, data: &Data<AppState>) -> Result<(), AppError> {
        // jogga's AnnounceQueries::upsert takes id: Uuid as first arg.
        if let Err(e) = crate::db::queries::AnnounceQueries::upsert(
            &data.db,
            Uuid::now_v7(),
            self.actor.as_str(),
            self.object.as_str(),
            self.id.as_str(),
        )
        .await
        {
            warn!(announce_id=%self.id, err=%e, "Announce: persist failed");
        }
        Ok(())
    }
}
