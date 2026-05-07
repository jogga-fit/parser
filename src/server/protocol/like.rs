use activitypub_federation::{config::Data, kinds::activity::LikeType, traits::Activity};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::db::queries::{ActorQueries, LikeQueries, NotificationQueries, ObjectQueries};
use crate::server::{error::AppError, state::AppState};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Like {
    #[serde(rename = "type")]
    pub kind: LikeType,
    pub id: Url,
    pub actor: Url,
    pub object: Url,
}

#[async_trait]
impl Activity for Like {
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
        if let Err(e) = LikeQueries::upsert(
            &data.db,
            self.actor.as_str(),
            self.object.as_str(),
            self.id.as_str(),
        )
        .await
        {
            tracing::warn!(like_id=%self.id, err=%e, "Like: persist failed");
        }

        // Insert a notification for the local owner of the liked object (non-fatal).
        {
            let db = data.db.clone();
            let actor_ap_id = self.actor.to_string();
            let object_ap_id = self.object.to_string();
            tokio::spawn(async move {
                let obj = match ObjectQueries::find_by_ap_id(&db, &object_ap_id).await {
                    Ok(o) => o,
                    Err(_) => return,
                };
                let owner_id = match obj.actor_id {
                    Some(id) => id,
                    None => return,
                };
                // Single-owner: check if the owner is the local account.
                let owner_is_local = sqlx::query_scalar!(
                    r#"SELECT EXISTS(SELECT 1 FROM local_accounts WHERE actor_id = ?) AS "exists!: i64""#,
                    owner_id,
                )
                .fetch_one(&db)
                .await
                .map(|n| n != 0)
                .unwrap_or(false);
                if !owner_is_local {
                    return;
                }
                let from_actor = match ActorQueries::find_by_ap_id(&db, &actor_ap_id).await {
                    Ok(a) => a,
                    Err(_) => return,
                };
                let title = obj
                    .ap_json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                if let Err(e) = NotificationQueries::insert(
                    &db,
                    owner_id,
                    "like",
                    from_actor.id,
                    Some(&object_ap_id),
                    title.as_deref(),
                )
                .await
                {
                    tracing::warn!(err=%e, "Like: notification insert failed");
                }
            });
        }

        Ok(())
    }
}
