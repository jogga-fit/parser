use activitypub_federation::{
    config::Data, fetch::object_id::ObjectId, kinds::activity::CreateType,
    protocol::verification::verify_domains_match, traits::Activity,
};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::db::queries::{
    ActivityQueries, ExerciseQueries, ObjectQueries, activity::NewActivity, exercise::NewExercise,
    object::NewObject,
};
use crate::server::{
    error::{AppError, InternalError},
    impls::actor::DbActor,
    protocol::exercise::Exercise,
    state::AppState,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExercise {
    #[serde(rename = "type")]
    pub kind: CreateType,
    pub id: Url,
    pub actor: ObjectId<DbActor>,
    pub object: Exercise,
    #[serde(
        default,
        deserialize_with = "activitypub_federation::protocol::helpers::deserialize_one_or_many"
    )]
    pub to: Vec<Url>,
    #[serde(
        default,
        deserialize_with = "activitypub_federation::protocol::helpers::deserialize_one_or_many"
    )]
    pub cc: Vec<Url>,
}

#[async_trait]
impl Activity for CreateExercise {
    type DataType = AppState;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.id
    }
    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, _data: &Data<AppState>) -> Result<(), AppError> {
        verify_domains_match(self.object.id.inner(), self.actor.inner())
            .map_err(|e| AppError::Internal(InternalError::Federation(e.to_string())))?;
        Ok(())
    }

    async fn receive(self, data: &Data<AppState>) -> Result<(), AppError> {
        let note = &self.object;
        let ap_id = note.id.inner().as_str();

        // Idempotent.
        if ObjectQueries::find_by_ap_id(&data.db, ap_id).await.is_ok() {
            return Ok(());
        }

        let actor_url = self.actor.inner().as_str();

        let actor_row =
            match crate::db::queries::ActorQueries::find_by_ap_id(&data.db, actor_url).await {
                Ok(r) => r,
                Err(crate::db::DbError::NotFound) => {
                    let actor_id: ObjectId<DbActor> = ObjectId::from(self.actor.inner().clone());
                    match actor_id.dereference(data).await {
                        Ok(db_actor) => db_actor.row,
                        Err(e) => {
                            tracing::warn!(
                                actor = %actor_url,
                                err = %e,
                                "CreateExercise: remote actor fetch failed, skipping"
                            );
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        actor = %actor_url,
                        err = %e,
                        "CreateExercise: actor lookup error, skipping"
                    );
                    return Ok(());
                }
            };

        let ap_json = serde_json::to_value(note)
            .map_err(|e| AppError::Internal(InternalError::DataIntegrity(e.to_string())))?;
        let obj = NewObject {
            ap_id,
            object_type: "Exercise",
            attributed_to: actor_url,
            actor_id: Some(actor_row.id),
            content: None,
            content_map: None,
            summary: None,
            sensitive: false,
            in_reply_to: None,
            published: note.published,
            url: None,
            ap_json,
            visibility: "public",
        };

        let started_at = note.started_at.or(note.published).unwrap_or_else(Utc::now);
        let ex = NewExercise {
            id: Uuid::now_v7(),
            actor_id: actor_row.id,
            activity_type: note.activity_type.clone(),
            started_at,
            duration_s: 0,
            distance_m: 0.0,
            elevation_gain_m: None,
            avg_pace_s_per_km: None,
            avg_heart_rate_bpm: None,
            max_heart_rate_bpm: None,
            avg_cadence_rpm: None,
            avg_power_w: None,
            max_power_w: None,
            normalized_power_w: None,
            title: note.name.clone(),
            file_type: "manual".to_string(),
            device: None,
            gpx_url: None,
            route: None,
            visibility: "public".to_string(),
            hidden_stats: vec![],
        };

        let activity_json = serde_json::to_value(&self)
            .map_err(|e| AppError::Internal(InternalError::DataIntegrity(e.to_string())))?;

        let activity = {
            let mut tx = match data.db.begin().await {
                Ok(tx) => tx,
                Err(e) => {
                    tracing::warn!(ap_id=%ap_id, err=%e, "CreateExercise: begin tx failed, skipping");
                    return Ok(());
                }
            };

            let object_id = match ExerciseQueries::insert_with_object(&mut tx, &obj, &ex).await {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!(ap_id=%ap_id, err=%e, "CreateExercise: persist failed, skipping");
                    return Ok(());
                }
            };

            let activity = match ActivityQueries::insert_tx(
                &mut tx,
                &NewActivity {
                    ap_id: self.id.to_string(),
                    activity_type: "Create".to_owned(),
                    actor_id: actor_row.id,
                    object_ap_id: ap_id.to_owned(),
                    target_ap_id: None,
                    object_id: Some(object_id),
                    ap_json: activity_json,
                },
            )
            .await
            {
                Ok(a) => a,
                Err(e) => {
                    tracing::warn!(ap_id=%ap_id, err=%e, "CreateExercise: activity insert failed, skipping");
                    return Ok(());
                }
            };

            if let Err(e) = tx.commit().await {
                tracing::warn!(ap_id=%ap_id, err=%e, "CreateExercise: commit failed, skipping");
                return Ok(());
            }

            activity
        };

        tracing::info!(
            ap_id = %ap_id,
            actor = %actor_url,
            activity_type = %note.activity_type,
            "CreateExercise: stored"
        );

        let local_followers = ActivityQueries::local_followers_of(&data.db, actor_url)
            .await
            .map_err(AppError::from)?;
        for follower_id in local_followers {
            if let Err(e) = ActivityQueries::add_to_inbox(&data.db, follower_id, activity.id).await
            {
                tracing::warn!(err=%e, "CreateExercise: inbox fan-out failed");
            }
        }

        Ok(())
    }
}
