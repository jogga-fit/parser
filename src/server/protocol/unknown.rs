use activitypub_federation::{config::Data, traits::Activity};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::server::{error::AppError, state::AppState};

/// Catch-all for activity types not handled by any other variant.
///
/// Satisfies the ActivityPub open-world assumption: unrecognized activities must
/// be accepted (202) rather than rejected.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Unknown {
    pub id: Url,
    pub actor: Url,
}

#[async_trait]
impl Activity for Unknown {
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

    async fn receive(self, _data: &Data<AppState>) -> Result<(), AppError> {
        tracing::debug!(id=%self.id, "ignoring unrecognised activity type");
        Ok(())
    }
}
