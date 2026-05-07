use activitypub_federation::{config::Data, kinds::activity::UpdateType, traits::Activity};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::debug;
use url::Url;

use crate::db::queries::ObjectQueries;
use crate::server::{error::AppError, state::AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    #[serde(rename = "type")]
    pub kind: UpdateType,
    pub id: Url,
    pub actor: Url,
    pub object: serde_json::Value,
}

#[async_trait]
impl Activity for Update {
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
        let object_id = match self
            .object
            .get("id")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
        {
            Some(id) => id,
            None => return Ok(()),
        };
        debug!(object_id, actor = %self.actor, "Update received");
        let obj = match ObjectQueries::find_by_ap_id(&data.db, &object_id).await {
            Ok(o) => o,
            Err(_) => return Ok(()),
        };
        if obj.attributed_to != self.actor.to_string() {
            return Ok(());
        }
        let content = self
            .object
            .get("content")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let summary = self
            .object
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        ObjectQueries::update_post(
            &data.db,
            &object_id,
            content.as_deref(),
            summary.as_deref(),
            self.object,
        )
        .await?;
        Ok(())
    }
}
