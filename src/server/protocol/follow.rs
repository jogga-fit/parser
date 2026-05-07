use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::activity::FollowType,
    traits::{Activity, Actor},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;
use url::Url;

use crate::db::queries::{FollowQueries, NotificationQueries};
use crate::server::{
    error::AppError, impls::actor::DbActor, protocol::accept::Accept, state::AppState,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Follow {
    #[serde(rename = "type")]
    pub kind: FollowType,
    pub id: Url,
    pub actor: ObjectId<DbActor>,
    pub object: ObjectId<DbActor>,
}

impl Follow {
    pub fn new(actor: ObjectId<DbActor>, object: ObjectId<DbActor>, id: Url) -> Self {
        Self {
            kind: FollowType::Follow,
            id,
            actor,
            object,
        }
    }
}

#[async_trait]
impl Activity for Follow {
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
        let target = self.object.dereference(data).await?;

        if !target.row.is_local {
            return Ok(());
        }

        // If the target has moved, return 410 Gone so the sender knows.
        if let Some(new_id) = &target.row.moved_to {
            return Err(AppError::Gone(format!("actor has migrated to {new_id}")));
        }

        let auto_accept = !target.row.manually_approves_followers;

        let mut conn = data.db.acquire().await.map_err(crate::db::DbError::Sqlx)?;
        FollowQueries::add_follower(
            &mut conn,
            target.row.id,
            follower.row.id,
            auto_accept,
            Some(self.id.as_str()),
        )
        .await?;

        if auto_accept {
            FollowQueries::add_following(&data.db, follower.row.id, target.row.id).await?;
            FollowQueries::accept_following(&data.db, follower.row.id, target.row.id).await?;

            // Send Accept back to the follower.
            let scheme = data.app_data().config.instance.scheme();
            let accept_id: Url = format!(
                "{scheme}://{}/accepts/{}",
                data.domain(),
                uuid::Uuid::now_v7()
            )
            .parse()
            .map_err(AppError::from)?;
            let accept = Accept::new(ObjectId::from(target.ap_url()), self, accept_id);
            let inbox = follower.inbox();
            let data_clone = data.clone();
            let target_clone = target.clone();
            tokio::spawn(async move {
                if let Err(e) = target_clone.send(accept, vec![inbox], &data_clone).await {
                    tracing::warn!(err=%e, "Follow: Accept delivery failed");
                }
            });
        }

        let kind = if auto_accept {
            "new_follower"
        } else {
            "follow_request"
        };
        if let Err(e) =
            NotificationQueries::insert(&data.db, target.row.id, kind, follower.row.id, None, None)
                .await
        {
            tracing::warn!(err=%e, "Follow: notification insert failed");
        }

        info!(
            follower = %follower.row.ap_id,
            target = %target.row.ap_id,
            auto_accept,
            "Follow received"
        );
        Ok(())
    }
}
