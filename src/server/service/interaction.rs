//! Like / unlike service functions.

use activitypub_federation::config::Data;
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::queries::{ActorQueries, NotificationQueries, ObjectQueries};
use crate::server::{
    error::AppError,
    protocol::{like::Like, undo_like::UndoLike},
    state::AppState,
};

use super::helpers::{actor_inbox_url, fetch_local_actor};

#[tracing::instrument(skip(data), fields(actor_id = %actor_id, object = object_ap_id))]
pub async fn do_like(
    data: &Data<AppState>,
    actor_id: Uuid,
    object_ap_id: &str,
) -> Result<(), AppError> {
    let local_actor = fetch_local_actor(data, actor_id).await?;

    let object_url: url::Url = object_ap_id
        .parse()
        .map_err(|_| AppError::BadRequest("invalid object URL".into()))?;

    let scheme = data.app_data().config.instance.scheme();
    let domain = data.domain();
    let like_id: url::Url = format!("{scheme}://{domain}/activities/{}", Uuid::now_v7())
        .parse()
        .map_err(AppError::from)?;

    let like = Like {
        kind: activitypub_federation::kinds::activity::LikeType::Like,
        id: like_id.clone(),
        actor: local_actor.ap_url(),
        object: object_url,
    };

    let obj = ObjectQueries::find_by_ap_id(&data.db, object_ap_id)
        .await
        .map_err(|_| AppError::NotFound)?;
    let owner = ActorQueries::find_by_ap_id(&data.db, &obj.attributed_to)
        .await
        .map_err(|_| AppError::NotFound)?;
    let owner_id = owner.id;
    let owner_is_local = owner.is_local;
    let inbox = actor_inbox_url(&owner)?;

    local_actor.send(like, vec![inbox], data).await?;

    crate::db::queries::LikeQueries::upsert(
        &data.db,
        local_actor.row.ap_id.0.as_str(),
        object_ap_id,
        like_id.as_str(),
    )
    .await
    .map_err(AppError::from)?;

    if owner_is_local {
        let title = obj
            .ap_json
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        if let Err(e) = NotificationQueries::insert(
            &data.db,
            owner_id,
            "like",
            local_actor.row.id,
            Some(object_ap_id),
            title.as_deref(),
        )
        .await
        {
            warn!(err=%e, "do_like: notification insert failed");
        }
    }

    info!(
        actor = local_actor.row.username,
        object = object_ap_id,
        "Like sent"
    );
    Ok(())
}

#[tracing::instrument(skip(data), fields(actor_id = %actor_id, object = object_ap_id))]
pub async fn do_unlike(
    data: &Data<AppState>,
    actor_id: Uuid,
    object_ap_id: &str,
) -> Result<(), AppError> {
    let local_actor = fetch_local_actor(data, actor_id).await?;

    let object_url: url::Url = object_ap_id
        .parse()
        .map_err(|_| AppError::BadRequest("invalid object URL".into()))?;

    let scheme = data.app_data().config.instance.scheme();
    let domain = data.domain();
    let like_id: url::Url = format!("{scheme}://{domain}/activities/{}", Uuid::now_v7())
        .parse()
        .map_err(AppError::from)?;
    let undo_id: url::Url = format!("{scheme}://{domain}/activities/{}", Uuid::now_v7())
        .parse()
        .map_err(AppError::from)?;

    let like = Like {
        kind: activitypub_federation::kinds::activity::LikeType::Like,
        id: like_id,
        actor: local_actor.ap_url(),
        object: object_url,
    };
    let undo = UndoLike {
        kind: activitypub_federation::kinds::activity::UndoType::Undo,
        id: undo_id,
        actor: local_actor.ap_url(),
        object: like,
    };

    let obj = ObjectQueries::find_by_ap_id(&data.db, object_ap_id)
        .await
        .map_err(|_| AppError::NotFound)?;
    let owner = ActorQueries::find_by_ap_id(&data.db, &obj.attributed_to)
        .await
        .map_err(|_| AppError::NotFound)?;
    let owner_id = owner.id;
    let owner_is_local = owner.is_local;
    let inbox = actor_inbox_url(&owner)?;

    local_actor.send(undo, vec![inbox], data).await?;

    crate::db::queries::LikeQueries::delete(
        &data.db,
        local_actor.row.ap_id.0.as_str(),
        object_ap_id,
    )
    .await
    .map_err(AppError::from)?;

    if owner_is_local {
        if let Err(e) =
            NotificationQueries::delete_like(&data.db, owner_id, local_actor.row.id, object_ap_id)
                .await
        {
            warn!(err=%e, "do_unlike: notification delete failed");
        }
    }

    info!(
        actor = local_actor.row.username,
        object = object_ap_id,
        "Unlike sent"
    );
    Ok(())
}
