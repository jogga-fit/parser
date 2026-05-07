use activitypub_federation::{
    config::Data, fetch::object_id::ObjectId, kinds::activity::RejectType, traits::Activity,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;
use url::Url;

use crate::db::queries::FollowQueries;
use crate::server::{
    error::AppError, impls::actor::DbActor, protocol::follow::Follow, state::AppState,
};

/// Sent by a remote actor when they reject a pending follow request.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reject {
    #[serde(rename = "type")]
    pub kind: RejectType,
    pub id: Url,
    pub actor: ObjectId<DbActor>,
    pub object: Follow,
}

impl Reject {
    pub fn new(actor: ObjectId<DbActor>, object: Follow, id: Url) -> Self {
        Self {
            kind: RejectType::Reject,
            id,
            actor,
            object,
        }
    }
}

#[async_trait]
impl Activity for Reject {
    type DataType = AppState;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.id
    }
    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, _data: &Data<AppState>) -> Result<(), AppError> {
        Ok(())
    }

    async fn receive(self, data: &Data<AppState>) -> Result<(), AppError> {
        let rejecting_actor = self.actor.dereference(data).await?;
        let local_actor = self.object.actor.dereference(data).await?;

        if !local_actor.row.is_local {
            return Ok(());
        }

        FollowQueries::remove_following(&data.db, local_actor.row.id, rejecting_actor.row.id)
            .await?;
        info!(
            local_actor = %local_actor.row.username,
            rejected_by = %rejecting_actor.row.ap_id,
            "Follow rejected by remote actor"
        );
        Ok(())
    }
}
