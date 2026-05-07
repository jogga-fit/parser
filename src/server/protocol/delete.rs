use activitypub_federation::{config::Data, kinds::activity::DeleteType, traits::Activity};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::debug;
use url::Url;

use crate::db::queries::ObjectQueries;
use crate::server::{error::AppError, state::AppState};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Delete {
    #[serde(rename = "type")]
    pub kind: DeleteType,
    pub id: Url,
    pub actor: Url,
    /// The AP ID of the object being deleted (string or object with `id`).
    pub object: DeleteObject,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DeleteObject {
    Url(Url),
    WithId { id: Url },
}

impl DeleteObject {
    fn url(&self) -> &Url {
        match self {
            DeleteObject::Url(u) => u,
            DeleteObject::WithId { id } => id,
        }
    }
}

#[async_trait]
impl Activity for Delete {
    type DataType = AppState;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.id
    }
    fn actor(&self) -> &Url {
        &self.actor
    }

    async fn verify(&self, _data: &Data<AppState>) -> Result<(), AppError> {
        let actor_host = self.actor.host_str().ok_or(AppError::BadRequest("invalid actor URL".into()))?;
        let object_host = self.object.url().host_str().ok_or(AppError::BadRequest("invalid object URL".into()))?;
        if actor_host != object_host {
            return Err(AppError::BadRequest(format!(
                "actor domain {actor_host} cannot delete object from {object_host}"
            )));
        }
        Ok(())
    }

    async fn receive(self, data: &Data<AppState>) -> Result<(), AppError> {
        let object_id = self.object.url().as_str();
        debug!(object_id, actor = %self.actor, "Delete received");
        let _ = ObjectQueries::delete_by_ap_id(&data.db, object_id).await;
        Ok(())
    }
}
