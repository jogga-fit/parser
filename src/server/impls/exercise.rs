use activitypub_federation::{
    config::Data, fetch::object_id::ObjectId, protocol::verification::verify_domains_match,
    traits::Object,
};
use async_trait::async_trait;
use chrono::Utc;
use url::Url;
use uuid::Uuid;

use crate::db::{
    models::ExerciseRow,
    queries::{ExerciseQueries, ObjectQueries, exercise::NewExercise, object::NewObject},
};
use crate::server::{
    error::{AppError, InternalError},
    impls::actor::DbActor,
    protocol::exercise::{Exercise, ExerciseType},
    state::AppState,
};

/// Wrapper around `ExerciseRow` for AP trait impls.
#[derive(Debug, Clone)]
pub struct DbExercise {
    pub row: ExerciseRow,
    pub ap_id: Url,
}

#[async_trait]
impl Object for DbExercise {
    type DataType = AppState;
    type Kind = Exercise;
    type Error = AppError;

    fn id(&self) -> &Url {
        &self.ap_id
    }

    async fn read_from_id(object_id: Url, data: &Data<AppState>) -> Result<Option<Self>, AppError> {
        match ExerciseQueries::find_by_ap_id(&data.db, object_id.as_str()).await {
            Ok(row) => Ok(Some(DbExercise {
                ap_id: object_id,
                row,
            })),
            Err(crate::db::DbError::NotFound) => Ok(None),
            Err(e) => Err(AppError::from(e)),
        }
    }

    async fn into_json(self, data: &Data<AppState>) -> Result<Exercise, AppError> {
        let domain = data.domain();
        let scheme = data.app_data().config.instance.scheme();
        let row = &self.row;
        let b58 = crate::server::id::encode(row.id);

        let route_url = format!("{scheme}://{domain}/api/exercises/{b58}/route")
            .parse::<Url>()
            .map_err(AppError::from)?;

        let stats_url = format!("{scheme}://{domain}/api/exercises/{b58}/stats")
            .parse::<Url>()
            .map_err(AppError::from)?;

        let attributed_to: Url = row.actor_ap_id.parse().map_err(AppError::from)?;

        Ok(Exercise {
            kind: ExerciseType::Exercise,
            id: ObjectId::from(self.ap_id.clone()),
            attributed_to: ObjectId::from(attributed_to),
            activity_type: row.activity_type.clone(),
            started_at: Some(row.started_at),
            name: row.title.clone(),
            content: None,
            route_url: Some(route_url),
            stats_url: Some(stats_url),
            published: Some(row.created_at),
            to: vec![],
            cc: vec![],
            attachment: vec![],
        })
    }

    async fn verify(
        json: &Exercise,
        expected_domain: &Url,
        _data: &Data<AppState>,
    ) -> Result<(), AppError> {
        verify_domains_match(json.id.inner(), expected_domain)
            .map_err(|e| AppError::Internal(InternalError::Federation(e.to_string())))?;
        Ok(())
    }

    async fn from_json(json: Exercise, data: &Data<AppState>) -> Result<Self, AppError> {
        let ap_id = json.id.inner().clone();
        let actor_url = json.attributed_to.inner().as_str();

        let actor_row =
            match crate::db::queries::ActorQueries::find_by_ap_id(&data.db, actor_url).await {
                Ok(r) => r,
                Err(crate::db::DbError::NotFound) => {
                    let actor_id: ObjectId<DbActor> =
                        ObjectId::from(json.attributed_to.inner().clone());
                    actor_id.dereference(data).await?.row
                }
                Err(e) => return Err(AppError::from(e)),
            };

        let actor_id = actor_row.id;

        let ap_json = serde_json::to_value(&json)
            .map_err(|e| AppError::Internal(InternalError::DataIntegrity(e.to_string())))?;
        let obj = NewObject {
            ap_id: ap_id.as_str(),
            object_type: "Exercise",
            attributed_to: actor_url,
            actor_id: Some(actor_id),
            content: json.content.as_deref(),
            content_map: None,
            summary: None,
            sensitive: false,
            in_reply_to: None,
            published: json.published,
            url: None,
            ap_json,
            visibility: "public",
        };

        let started_at = json.started_at.or(json.published).unwrap_or_else(Utc::now);
        let ex_id = Uuid::now_v7();
        let ex = NewExercise {
            id: ex_id,
            actor_id,
            activity_type: json.activity_type.clone(),
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
            title: json.name.clone(),
            file_type: "unknown".to_string(),
            device: None,
            gpx_url: None,
            route: None,
            visibility: "public".to_string(),
            hidden_stats: vec![],
        };

        let mut tx = data
            .db
            .begin()
            .await
            .map_err(|e| AppError::from(crate::db::DbError::Sqlx(e)))?;
        ExerciseQueries::insert_with_object(&mut tx, &obj, &ex).await?;
        tx.commit()
            .await
            .map_err(|e| AppError::from(crate::db::DbError::Sqlx(e)))?;

        let row = ExerciseQueries::find_metadata_by_id(&data.db, ex_id).await?;
        Ok(DbExercise { ap_id, row })
    }
}

impl DbExercise {
    /// Look up an exercise by UUID, also loading the ap_id from the objects table.
    pub async fn find_by_id(id: Uuid, data: &Data<AppState>) -> Result<Option<Self>, AppError> {
        let row = match ExerciseQueries::find_metadata_by_id(&data.db, id).await {
            Ok(r) => r,
            Err(crate::db::DbError::NotFound) => return Ok(None),
            Err(e) => return Err(AppError::from(e)),
        };
        let obj = match ObjectQueries::find_by_id(&data.db, row.object_id).await {
            Ok(o) => o,
            Err(crate::db::DbError::NotFound) => return Ok(None),
            Err(e) => return Err(AppError::from(e)),
        };
        // ObjectRow.ap_id is String in jogga.
        let ap_id: Url = obj.ap_id.parse().map_err(AppError::from)?;
        Ok(Some(DbExercise { ap_id, row }))
    }
}
