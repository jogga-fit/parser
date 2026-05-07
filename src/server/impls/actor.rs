use std::fmt::Debug;

use activitypub_federation::{
    activity_sending::SendActivityTask,
    config::Data,
    protocol::{context::WithContext, public_key::PublicKey, verification::verify_domains_match},
    traits::{Activity, Actor, Object},
};
use async_trait::async_trait;
use serde::Serialize;
use tracing::{debug, error};
use url::Url;

use crate::db::{
    models::ActorRow,
    queries::{ActorQueries, actor::NewActor},
};
use crate::server::{
    error::{AppError, InternalError},
    protocol::person::{Endpoints, RemoteActor},
    state::AppState,
};

/// Thin wrapper around `ActorRow` so we can implement foreign traits on it.
#[derive(Debug, Clone)]
pub struct DbActor {
    pub row: ActorRow,
}

impl DbActor {
    pub fn ap_url(&self) -> Url {
        self.row.ap_id.0.clone()
    }

    pub async fn send<A>(
        &self,
        activity: A,
        recipients: Vec<Url>,
        data: &Data<AppState>,
    ) -> Result<(), AppError>
    where
        A: Activity + Serialize + Debug + Send + Sync,
    {
        debug!(
            actor = %self.row.ap_id,
            recipient_count = recipients.len(),
            "sending AP activity"
        );
        let wrapped = WithContext::new_default(activity);
        let tasks = SendActivityTask::prepare(&wrapped, self, recipients, data).await?;
        for task in tasks {
            task.sign_and_send(data).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Object for DbActor {
    type DataType = AppState;
    /// Use `RemoteActor` so we can dereference both `Person` and `Group` APs.
    type Kind = RemoteActor;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.row.ap_id.0
    }

    async fn read_from_id(object_id: Url, data: &Data<AppState>) -> Result<Option<Self>, AppError> {
        match ActorQueries::find_by_ap_id(&data.db, object_id.as_str()).await {
            Ok(row) => Ok(Some(DbActor { row })),
            Err(crate::db::DbError::NotFound) => Ok(None),
            Err(e) => Err(AppError::from(e)),
        }
    }

    /// Serialise a local `Person` actor for federation (outbound).
    async fn into_json(self, data: &Data<AppState>) -> Result<RemoteActor, AppError> {
        let scheme = data.app_data().config.instance.scheme();
        let ActorRow {
            id: _,
            ap_id,
            username,
            domain,
            display_name,
            summary,
            public_key_pem,
            inbox_url,
            outbox_url,
            followers_url,
            following_url,
            manually_approves_followers,
            also_known_as,
            moved_to,
            ..
        } = self.row;
        let base = format!("{scheme}://{domain}/users/{username}");

        Ok(RemoteActor {
            kind: "Person".to_string(),
            id: ap_id.0.clone(),
            preferred_username: username,
            name: display_name,
            summary,
            inbox: inbox_url.parse().map_err(AppError::from)?,
            outbox: outbox_url.parse().map_err(AppError::from)?,
            followers: followers_url.parse().map_err(AppError::from)?,
            following: Some(following_url.parse().map_err(AppError::from)?),
            endpoints: Some(Endpoints {
                shared_inbox: Some(
                    format!("{scheme}://{domain}/inbox")
                        .parse()
                        .map_err(AppError::from)?,
                ),
            }),
            public_key: PublicKey {
                id: format!("{base}#main-key"),
                owner: ap_id.0,
                public_key_pem: public_key_pem.clone(),
            },
            manually_approves_followers,
            also_known_as: also_known_as
                .iter()
                .filter_map(|s| s.parse().ok())
                .collect(),
            moved_to: moved_to.as_deref().and_then(|s| s.parse().ok()),
        })
    }

    async fn verify(
        json: &RemoteActor,
        expected_domain: &Url,
        _data: &Data<AppState>,
    ) -> Result<(), AppError> {
        verify_domains_match(&json.id, expected_domain)
            .map_err(|e| AppError::Internal(InternalError::Federation(e.to_string())))?;
        Ok(())
    }

    async fn from_json(json: RemoteActor, data: &Data<AppState>) -> Result<Self, AppError> {
        let domain = {
            match json.id.port() {
                Some(p) => format!("{}:{}", json.id.host_str().unwrap_or(""), p),
                None => json.id.host_str().unwrap_or("").to_owned(),
            }
        };
        let also_known_as: Vec<String> = json.also_known_as.iter().map(|u| u.to_string()).collect();
        let moved_to = json.moved_to.as_ref().map(|u| u.to_string());
        let new_actor = NewActor {
            ap_id: json.id.as_str(),
            username: &json.preferred_username,
            domain: &domain,
            actor_type: &json.kind,
            display_name: json.name.as_deref(),
            summary: json.summary.as_deref(),
            public_key_pem: &json.public_key.public_key_pem,
            private_key_pem: None,
            inbox_url: json.inbox.as_str(),
            outbox_url: json.outbox.as_str(),
            followers_url: json.followers.as_str(),
            following_url: json.following.as_ref().map(|u| u.as_str()).unwrap_or(""),
            shared_inbox_url: json
                .endpoints
                .as_ref()
                .and_then(|e| e.shared_inbox.as_ref())
                .map(|u| u.as_str()),
            manually_approves_followers: json.manually_approves_followers,
            is_local: false,
            ap_json: None,
            also_known_as: &also_known_as,
            moved_to: moved_to.as_deref(),
        };
        let row = ActorQueries::upsert_remote(&data.db, &new_actor)
            .await
            .map_err(|e| {
                error!(
                    ap_id = new_actor.ap_id,
                    username = new_actor.username,
                    domain = new_actor.domain,
                    error = %e,
                    "from_json: upsert_remote failed"
                );
                AppError::from(e)
            })?;
        Ok(DbActor { row })
    }
}

impl Actor for DbActor {
    fn public_key_pem(&self) -> &str {
        &self.row.public_key_pem
    }

    fn private_key_pem(&self) -> Option<String> {
        self.row.private_key_pem.clone()
    }

    fn inbox(&self) -> Url {
        self.row
            .inbox_url
            .parse()
            .expect("inbox_url is always valid")
    }

    fn shared_inbox(&self) -> Option<Url> {
        self.row
            .shared_inbox_url
            .as_ref()
            .and_then(|u| u.parse().ok())
    }
}
