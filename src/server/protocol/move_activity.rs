use activitypub_federation::{config::Data, fetch::object_id::ObjectId, traits::Activity};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use url::Url;

use crate::db::queries::{ActorQueries, FollowQueries};
use crate::server::{error::AppError, impls::actor::DbActor, state::AppState};

/// ActivityPub `Move` activity for account migration.
///
/// Mastodon-compatible semantics: `actor` and `object` MUST be the same actor
/// (the one being migrated). `target` MUST list `object` in its `alsoKnownAs`
/// to prevent hijack.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Move {
    #[serde(rename = "type")]
    pub kind: MoveType,
    pub id: Url,
    pub actor: ObjectId<DbActor>,
    pub object: ObjectId<DbActor>,
    pub target: ObjectId<DbActor>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MoveType {
    Move,
}

#[async_trait]
impl Activity for Move {
    type DataType = AppState;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.id
    }

    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, data: &Data<AppState>) -> Result<(), AppError> {
        if self.actor.inner() != self.object.inner() {
            return Err(AppError::BadRequest(
                "Move.actor must equal Move.object".into(),
            ));
        }
        if self.object.inner() == self.target.inner() {
            return Err(AppError::BadRequest(
                "Move target must differ from object".into(),
            ));
        }
        let target = self.target.dereference(data).await?;
        let origin = self.object.inner().to_string();
        if !target.row.also_known_as.iter().any(|a| a == &origin) {
            return Err(AppError::BadRequest(
                "target actor does not claim this actor in alsoKnownAs".into(),
            ));
        }
        Ok(())
    }

    async fn receive(self, data: &Data<AppState>) -> Result<(), AppError> {
        let pool = &data.app_data().db;
        let object = self.object.dereference(data).await?;
        let target = self.target.dereference(data).await?;

        ActorQueries::set_moved_to(pool, object.row.id, target.ap_url().as_str()).await?;

        let local_followers = FollowQueries::list_local_followers_of(pool, object.row.id).await?;
        let target_ap = target.ap_url().to_string();

        info!(
            origin = %object.row.ap_id,
            target = %target.row.ap_id,
            local_followers = local_followers.len(),
            "Move received: redirecting local followers"
        );

        for follower in local_followers {
            if let Err(e) =
                crate::server::service::do_follow(data, follower.id, &target_ap, false).await
            {
                warn!(
                    follower = %follower.ap_id,
                    target = %target_ap,
                    err = %e,
                    "Move: failed to redirect follower"
                );
                continue;
            }
            if let Err(e) = FollowQueries::remove_follower(pool, object.row.id, follower.id).await {
                warn!(
                    follower = %follower.ap_id,
                    origin = %object.row.ap_id,
                    err = %e,
                    "Move: failed to remove old follow row"
                );
            }
        }
        Ok(())
    }
}
