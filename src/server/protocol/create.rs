use activitypub_federation::{
    config::Data, fetch::object_id::ObjectId, kinds::activity::CreateType,
    protocol::helpers::deserialize_one_or_many, traits::Activity,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use url::Url;

use crate::db::{
    queries::activity::NewActivity,
    queries::object::NewObject,
    queries::{ActivityQueries, ActorQueries, NotificationQueries, ObjectQueries},
};
use crate::server::{
    error::AppError,
    impls::actor::DbActor,
    protocol::note::{Note, visibility_from_to_cc},
    state::AppState,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Create {
    #[serde(rename = "type")]
    pub kind: CreateType,
    pub id: Url,
    pub actor: ObjectId<DbActor>,
    pub object: Note,
    #[serde(default, deserialize_with = "deserialize_one_or_many")]
    pub to: Vec<Url>,
    #[serde(default, deserialize_with = "deserialize_one_or_many")]
    pub cc: Vec<Url>,
}

#[async_trait]
impl Activity for Create {
    type DataType = AppState;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.id
    }
    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, _data: &Data<AppState>) -> Result<(), AppError> {
        let actor_host = self.actor.inner().host_str().ok_or(AppError::BadRequest("invalid actor URL".into()))?;
        let note_host = self.object.id.inner().host_str().ok_or(AppError::BadRequest("invalid note URL".into()))?;
        if actor_host != note_host {
            return Err(AppError::BadRequest(format!(
                "actor domain {actor_host} cannot create object attributed to {note_host}"
            )));
        }
        Ok(())
    }

    async fn receive(self, data: &Data<AppState>) -> Result<(), AppError> {
        let sender = self.actor.dereference(data).await?;
        let actor_ap_id = sender.ap_url().to_string();
        let note = &self.object;
        let note_ap_id = note.id.inner().as_str();

        let vis = visibility_from_to_cc(&note.to, &note.cc);
        let new_obj = NewObject {
            ap_id: note_ap_id,
            object_type: "Note",
            attributed_to: &actor_ap_id,
            actor_id: None,
            content: Some(note.content.as_str()),
            content_map: None,
            summary: note.summary.as_deref(),
            sensitive: note.sensitive,
            in_reply_to: note.in_reply_to.as_ref().map(|u| u.as_str()),
            published: None,
            url: note.url.as_ref().map(|u| u.as_str()),
            ap_json: serde_json::to_value(note).unwrap_or(serde_json::Value::Null),
            visibility: vis,
        };
        if let Err(e) = ObjectQueries::insert(&data.db, &new_obj).await {
            warn!(err=%e, note_id=note_ap_id, "Create::receive: failed to insert object — skipping");
            return Ok(());
        }

        let activity_json = serde_json::to_value(&self).unwrap_or(serde_json::Value::Null);
        let activity_ap_id = self.id.as_str();
        let activity = match ActivityQueries::find_by_ap_id(&data.db, activity_ap_id).await {
            Ok(existing) => existing,
            Err(_) => {
                let new_act = NewActivity {
                    ap_id: activity_ap_id.to_owned(),
                    activity_type: "Create".to_owned(),
                    actor_id: sender.row.id,
                    object_ap_id: note_ap_id.to_owned(),
                    target_ap_id: None,
                    object_id: None,
                    ap_json: activity_json,
                };
                match ActivityQueries::insert(&data.db, &new_act).await {
                    Ok(a) => a,
                    Err(e) => {
                        warn!(err=%e, "Create::receive: failed to insert activity");
                        return Ok(());
                    }
                }
            }
        };

        let local_followers = ActivityQueries::local_followers_of(&data.db, &actor_ap_id)
            .await
            .unwrap_or_default();
        let follower_count = local_followers.len();
        for follower_id in local_followers {
            if let Err(e) = ActivityQueries::add_to_inbox(&data.db, follower_id, activity.id).await {
                warn!(err=%e, follower_id=%follower_id, "Create::receive: inbox fan-out failed for follower");
            }
        }

        debug!(
            note_id = note_ap_id,
            actor = actor_ap_id,
            fanned_to = follower_count,
            "Create received: note stored and fanned into local inboxes"
        );

        // Notify local owner of parent if this is a reply.
        if let Some(parent_ap_id) = note.in_reply_to.as_ref().map(|u| u.to_string()) {
            let db = data.db.clone();
            let replier_ap_id = actor_ap_id.clone();
            tokio::spawn(async move {
                let parent = match ObjectQueries::find_by_ap_id(&db, &parent_ap_id).await {
                    Ok(o) => o,
                    Err(_) => return,
                };
                let owner_id = match parent.actor_id {
                    Some(id) => id,
                    None => return,
                };
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
                let from_actor = match ActorQueries::find_by_ap_id(&db, &replier_ap_id).await {
                    Ok(a) => a,
                    Err(_) => return,
                };
                if from_actor.id == owner_id {
                    return;
                }
                if let Err(e) = NotificationQueries::insert(
                    &db,
                    owner_id,
                    "reply",
                    from_actor.id,
                    Some(&parent_ap_id),
                    None,
                )
                .await
                {
                    warn!(err=%e, "Create: reply notification insert failed");
                }
            });
        }

        Ok(())
    }
}
