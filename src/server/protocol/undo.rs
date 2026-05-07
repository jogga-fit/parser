use activitypub_federation::{
    config::Data, fetch::object_id::ObjectId, kinds::activity::UndoType, traits::Activity,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::debug;
use url::Url;

use crate::db::queries::FollowQueries;
use crate::server::{
    error::AppError, impls::actor::DbActor, protocol::follow::Follow, state::AppState,
};

/// Generic Undo — only Follow is handled; others are ignored.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Undo {
    #[serde(rename = "type")]
    pub kind: UndoType,
    pub id: Url,
    pub actor: ObjectId<DbActor>,
    pub object: Follow,
}

#[async_trait]
impl Activity for Undo {
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
        let follower = self.actor.dereference(data).await?;
        let target = self.object.object.dereference(data).await?;
        let _ = FollowQueries::remove_follower(&data.db, target.row.id, follower.row.id).await;
        debug!(
            follower = %follower.row.ap_id,
            target = %target.row.ap_id,
            "Undo Follow: follower removed"
        );
        Ok(())
    }
}
