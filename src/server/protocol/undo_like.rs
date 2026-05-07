use activitypub_federation::{config::Data, kinds::activity::UndoType, traits::Activity};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::db::queries::{ActorQueries, LikeQueries, NotificationQueries, ObjectQueries};
use crate::server::{error::AppError, state::AppState};

use super::like::Like;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoLike {
    #[serde(rename = "type")]
    pub kind: UndoType,
    pub id: Url,
    pub actor: Url,
    pub object: Like,
}

#[async_trait]
impl Activity for UndoLike {
    type DataType = AppState;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.id
    }
    fn actor(&self) -> &Url {
        &self.actor
    }

    async fn verify(&self, _data: &Data<AppState>) -> Result<(), AppError> {
        Ok(())
    }

    async fn receive(self, data: &Data<AppState>) -> Result<(), AppError> {
        let actor_ap_id = self.actor.to_string();
        let object_ap_id = self.object.object.to_string();

        if let Err(e) = LikeQueries::delete(&data.db, &actor_ap_id, &object_ap_id).await {
            tracing::warn!(undo_id=%self.id, err=%e, "UndoLike: delete failed");
        }

        // Remove the like notification (non-fatal).
        {
            let db = data.db.clone();
            tokio::spawn(async move {
                let obj = match ObjectQueries::find_by_ap_id(&db, &object_ap_id).await {
                    Ok(o) => o,
                    Err(_) => return,
                };
                let owner_id = match obj.actor_id {
                    Some(id) => id,
                    None => return,
                };
                let from_actor = match ActorQueries::find_by_ap_id(&db, &actor_ap_id).await {
                    Ok(a) => a,
                    Err(_) => return,
                };
                if let Err(e) =
                    NotificationQueries::delete_like(&db, owner_id, from_actor.id, &object_ap_id)
                        .await
                {
                    tracing::warn!(err=%e, "UndoLike: notification delete failed");
                }
            });
        }

        Ok(())
    }
}
