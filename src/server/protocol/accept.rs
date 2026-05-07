use activitypub_federation::{
    config::Data, fetch::object_id::ObjectId, kinds::activity::AcceptType, traits::Activity,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;
use url::Url;

use crate::db::queries::{FollowQueries, NotificationQueries};
use crate::server::{
    error::AppError, impls::actor::DbActor, protocol::follow::Follow, state::AppState,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Accept {
    #[serde(rename = "type")]
    pub kind: AcceptType,
    pub id: Url,
    pub actor: ObjectId<DbActor>,
    pub object: Follow,
}

impl Accept {
    pub fn new(actor: ObjectId<DbActor>, object: Follow, id: Url) -> Self {
        Self {
            kind: AcceptType::Accept,
            id,
            actor,
            object,
        }
    }
}

#[async_trait]
impl Activity for Accept {
    type DataType = AppState;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.id
    }
    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, _data: &Data<AppState>) -> Result<(), AppError> {
        // The Accept must come from the actor who was followed, not a third party.
        if self.actor.inner() != self.object.object.inner() {
            return Err(AppError::BadRequest(
                "Accept actor must be the followed party".into()
            ));
        }
        Ok(())
    }

    async fn receive(self, data: &Data<AppState>) -> Result<(), AppError> {
        let accepting_actor = self.actor.dereference(data).await?;
        let local_actor = self.object.actor.dereference(data).await?;
        FollowQueries::accept_following(&data.db, local_actor.row.id, accepting_actor.row.id)
            .await?;
        info!(
            local_actor = %local_actor.row.username,
            accepted_by = %accepting_actor.row.ap_id,
            "Follow accepted by remote actor"
        );
        if local_actor.row.is_local {
            if let Err(e) = NotificationQueries::insert(
                &data.db,
                local_actor.row.id,
                "follow_accepted",
                accepting_actor.row.id,
                None,
                None,
            )
            .await
            {
                tracing::warn!(err=%e, "Accept: notification insert failed");
            }
        }
        Ok(())
    }
}
