//! Background delivery worker for outbound ActivityPub federation.
//!
//! Polls `outbox_deliveries` every 30 seconds, claims due rows, signs and sends
//! each activity, then marks the row `success` or schedules a retry with
//! exponential backoff.

use std::time::Duration;

use activitypub_federation::config::FederationConfig;
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};

use crate::db::queries::{ActivityQueries, ActorQueries, DeliveryQueries};
use crate::server::{
    impls::actor::DbActor,
    protocol::{
        announce::Announce, create::Create, create_exercise::CreateExercise, move_activity::Move,
    },
    state::AppState,
};

fn next_retry(attempt_count: i32) -> Option<DateTime<Utc>> {
    let minutes = match attempt_count {
        1 => 1,
        2 => 5,
        3 => 30,
        _ => return None,
    };
    Some(Utc::now() + chrono::Duration::minutes(minutes))
}

pub async fn run_delivery_worker(fed_config: FederationConfig<AppState>) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        let data = fed_config.to_request_data();
        process_due_deliveries(&data).await;
    }
}

async fn process_due_deliveries(data: &activitypub_federation::config::Data<AppState>) {
    let rows = match DeliveryQueries::claim_due_deliveries(&data.db, 50).await {
        Ok(r) => r,
        Err(e) => {
            warn!(err = %e, "delivery worker: claim_due_deliveries failed");
            return;
        }
    };

    if rows.is_empty() {
        return;
    }
    debug!(count = rows.len(), "delivery worker: processing deliveries");

    for delivery in rows {
        let activity_row = match ActivityQueries::find_by_uuid(&data.db, delivery.activity_id).await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(delivery_id = %delivery.id, err = %e, "delivery: activity not found");
                let _ =
                    DeliveryQueries::mark_failed(&data.db, delivery.id, &e.to_string(), None).await;
                continue;
            }
        };

        // jogga: ap_json is a JsonValue newtype — use .0 to get serde_json::Value.
        let ap_json = activity_row.ap_json.0.clone();

        let activity_type = ap_json
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("Unknown")
            .to_owned();
        let object_type = ap_json
            .get("object")
            .and_then(|o| o.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("Unknown")
            .to_owned();

        let actor_row = match ActorQueries::find_by_id(&data.db, activity_row.actor_id).await {
            Ok(r) => r,
            Err(e) => {
                warn!(delivery_id = %delivery.id, err = %e, "delivery: actor not found");
                let _ =
                    DeliveryQueries::mark_failed(&data.db, delivery.id, &e.to_string(), None).await;
                continue;
            }
        };

        let inbox: url::Url = match delivery.inbox_url.parse() {
            Ok(u) => u,
            Err(e) => {
                warn!(delivery_id = %delivery.id, url = %delivery.inbox_url, "delivery: invalid inbox URL");
                let _ =
                    DeliveryQueries::mark_failed(&data.db, delivery.id, &e.to_string(), None).await;
                continue;
            }
        };

        let db_actor = DbActor { row: actor_row };
        let send_result = if activity_type == "Announce" {
            match serde_json::from_value::<Announce>(ap_json) {
                Ok(act) => db_actor.send(act, vec![inbox], data).await,
                Err(e) => {
                    warn!(delivery_id = %delivery.id, err = %e, "delivery: Announce deserialize failed");
                    let _ =
                        DeliveryQueries::mark_failed(&data.db, delivery.id, &e.to_string(), None)
                            .await;
                    continue;
                }
            }
        } else if activity_type == "Move" {
            match serde_json::from_value::<Move>(ap_json) {
                Ok(act) => db_actor.send(act, vec![inbox], data).await,
                Err(e) => {
                    warn!(delivery_id = %delivery.id, err = %e, "delivery: Move deserialize failed");
                    let _ =
                        DeliveryQueries::mark_failed(&data.db, delivery.id, &e.to_string(), None)
                            .await;
                    continue;
                }
            }
        } else if object_type == "Note" {
            match serde_json::from_value::<Create>(ap_json) {
                Ok(act) => db_actor.send(act, vec![inbox], data).await,
                Err(e) => {
                    warn!(delivery_id = %delivery.id, err = %e, "delivery: Note deserialize failed");
                    let _ =
                        DeliveryQueries::mark_failed(&data.db, delivery.id, &e.to_string(), None)
                            .await;
                    continue;
                }
            }
        } else {
            match serde_json::from_value::<CreateExercise>(ap_json) {
                Ok(act) => db_actor.send(act, vec![inbox], data).await,
                Err(e) => {
                    warn!(delivery_id = %delivery.id, err = %e, "delivery: ap_json deserialize failed");
                    let _ =
                        DeliveryQueries::mark_failed(&data.db, delivery.id, &e.to_string(), None)
                            .await;
                    continue;
                }
            }
        };

        match send_result {
            Ok(()) => {
                info!(delivery_id = %delivery.id, "delivery succeeded");
                if let Err(e) = DeliveryQueries::mark_success(&data.db, delivery.id).await {
                    warn!(delivery_id = %delivery.id, err = %e, "delivery: mark_success failed — will retry");
                }
            }
            Err(e) => {
                let retry_at = next_retry(delivery.attempt_count);
                warn!(
                    delivery_id = %delivery.id,
                    attempt     = delivery.attempt_count,
                    permanent   = retry_at.is_none(),
                    err         = %e,
                    "delivery failed"
                );
                let _ =
                    DeliveryQueries::mark_failed(&data.db, delivery.id, &e.to_string(), retry_at)
                        .await;
            }
        }
    }
}
