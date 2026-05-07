//! Social graph service functions: follow, unfollow, alias, move, resolve.

use activitypub_federation::{config::Data, fetch::object_id::ObjectId, traits::Actor};
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::queries::{
    AccountQueries, ActivityQueries, ActorQueries, DeliveryQueries, FollowQueries,
    NotificationQueries, activity::NewActivity,
};
use crate::server::{
    error::AppError,
    impls::actor::DbActor,
    protocol::{follow::Follow, undo::Undo},
    state::AppState,
};

use super::helpers::{actor_inbox_url, fetch_local_actor};

/// Resolve `@user@domain` or plain URL to an AP URL.
pub async fn resolve_handle(state: &AppState, handle: &str) -> Result<String, AppError> {
    let _ = state;
    // Trim leading @.
    let h = handle.trim_start_matches('@');
    // @user@domain → https://domain/users/user (naive fallback)
    if let Some((user, domain)) = h.split_once('@') {
        return Ok(format!("https://{domain}/users/{user}"));
    }
    Err(AppError::BadRequest("invalid handle format".into()))
}

/// Send a Follow activity and record the pending following.
#[tracing::instrument(skip(data), fields(actor_id = %actor_id, target = target_ap_id))]
pub async fn do_follow(
    data: &Data<AppState>,
    actor_id: Uuid,
    target_ap_id: &str,
    _person_only: bool,
) -> Result<(), AppError> {
    let target_url: url::Url = target_ap_id
        .parse()
        .map_err(|_| AppError::BadRequest("invalid actor URL".into()))?;

    let local_actor = fetch_local_actor(data, actor_id).await?;
    let target: DbActor = ObjectId::from(target_url).dereference(data).await?;

    if FollowQueries::is_following(&data.db, local_actor.row.id, target.row.id).await? {
        return Ok(());
    }

    let scheme = data.app_data().config.instance.scheme();
    let follow_id_str = format!("{scheme}://{}/activities/{}", data.domain(), Uuid::now_v7());
    let follow_id = follow_id_str.parse::<url::Url>().map_err(AppError::from)?;

    let follow = Follow::new(
        ObjectId::from(local_actor.ap_url()),
        ObjectId::from(target.ap_url()),
        follow_id,
    );

    let inbox = target.inbox();
    local_actor.send(follow, vec![inbox], data).await?;

    FollowQueries::add_following(&data.db, local_actor.row.id, target.row.id).await?;

    if target.row.is_local {
        let auto_accept = !target.row.manually_approves_followers;
        let mut conn = data.db.acquire().await.map_err(crate::db::DbError::Sqlx)?;
        FollowQueries::add_follower(
            &mut conn,
            target.row.id,
            local_actor.row.id,
            auto_accept,
            Some(follow_id_str.as_str()),
        )
        .await?;
        if auto_accept {
            FollowQueries::accept_following(&data.db, local_actor.row.id, target.row.id).await?;
        }
        let kind = if auto_accept {
            "new_follower"
        } else {
            "follow_request"
        };
        if let Err(e) = NotificationQueries::insert(
            &data.db,
            target.row.id,
            kind,
            local_actor.row.id,
            None,
            None,
        )
        .await
        {
            warn!(err=%e, "do_follow: notification insert failed");
        }
    }

    info!(
        from = local_actor.row.username,
        to = %target.row.ap_id,
        "Follow sent"
    );
    Ok(())
}

/// Send an Undo Follow and remove the following record.
#[tracing::instrument(skip(data), fields(actor_id = %actor_id, target = target_ap_id))]
pub async fn do_unfollow(
    data: &Data<AppState>,
    actor_id: Uuid,
    target_ap_id: &str,
) -> Result<(), AppError> {
    let target_url: url::Url = target_ap_id
        .parse()
        .map_err(|_| AppError::BadRequest("invalid actor URL".into()))?;

    let local_actor = fetch_local_actor(data, actor_id).await?;
    let target: DbActor = ObjectId::from(target_url).dereference(data).await?;

    if !FollowQueries::is_following(&data.db, local_actor.row.id, target.row.id).await? {
        return Ok(());
    }

    let scheme = data.app_data().config.instance.scheme();
    let domain = data.domain();

    let follow_id = format!("{scheme}://{domain}/activities/{}", Uuid::now_v7())
        .parse::<url::Url>()
        .map_err(AppError::from)?;
    let undo_id = format!("{scheme}://{domain}/activities/{}", Uuid::now_v7())
        .parse::<url::Url>()
        .map_err(AppError::from)?;

    let follow = Follow::new(
        ObjectId::from(local_actor.ap_url()),
        ObjectId::from(target.ap_url()),
        follow_id,
    );
    let undo = Undo {
        kind: activitypub_federation::kinds::activity::UndoType::Undo,
        id: undo_id,
        actor: ObjectId::from(local_actor.ap_url()),
        object: follow,
    };

    let inbox = target.inbox();
    local_actor.send(undo, vec![inbox], data).await?;

    FollowQueries::remove_following(&data.db, local_actor.row.id, target.row.id).await?;

    if target.row.is_local {
        FollowQueries::remove_follower(&data.db, target.row.id, local_actor.row.id).await?;
    }

    info!(
        from = local_actor.row.username,
        to = %target.row.ap_id,
        "Unfollow sent"
    );
    Ok(())
}

/// Add an `alsoKnownAs` entry.
pub async fn do_add_alias(
    data: &Data<AppState>,
    actor: &crate::db::models::ActorRow,
    also_known_as_str: &str,
) -> Result<(), AppError> {
    let also_url: url::Url = also_known_as_str
        .parse()
        .map_err(|_| AppError::BadRequest("invalid alsoKnownAs URL".into()))?;

    ActorQueries::add_alias(&data.db, actor.id, also_url.as_str()).await?;
    info!(actor_id = %actor.id, "alsoKnownAs added");
    Ok(())
}

/// Remove an `alsoKnownAs` entry.
pub async fn do_remove_alias(
    pool: &crate::db::SqlitePool,
    actor_id: Uuid,
    also_known_as_str: &str,
) -> Result<(), AppError> {
    ActorQueries::remove_alias(pool, actor_id, also_known_as_str).await?;
    info!(actor_id = %actor_id, "alsoKnownAs removed");
    Ok(())
}

/// Send a `Move` activity — account migration to a new AP actor.
pub async fn do_move_account(
    data: &Data<AppState>,
    actor: &crate::db::models::ActorRow,
    new_ap_id_str: &str,
) -> Result<(), AppError> {
    // Guard: already migrated.
    if actor.moved_to.is_some() {
        return Err(AppError::Conflict("account already migrated".into()));
    }

    let new_ap_id: url::Url = new_ap_id_str
        .parse()
        .map_err(|_| AppError::BadRequest("invalid target actor URL".into()))?;

    // Guard: cannot move to self.
    if new_ap_id.as_str() == actor.ap_id.0.as_str() {
        return Err(AppError::BadRequest("cannot move to self".into()));
    }

    // Guard: target must have this actor in its alsoKnownAs (anti-hijack check).
    // Look up the target in our local DB — if not cached, skip the check (let
    // federation handle it). If cached, enforce the constraint.
    let local_ap = actor.ap_id.0.as_str();
    if let Ok(target) = ActorQueries::find_by_ap_id(&data.db, new_ap_id.as_str()).await {
        if !target.also_known_as.iter().any(|a| a == local_ap) {
            return Err(AppError::BadRequest(
                "target actor must list this account in alsoKnownAs".into(),
            ));
        }
    }

    let actor_ap_id = actor.ap_id.0.as_str();
    let activity_ap_id = format!("{}#move-{}", actor_ap_id, Uuid::now_v7());
    let ap_json = serde_json::json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id":     activity_ap_id,
        "type":   "Move",
        "actor":  actor_ap_id,
        "object": actor_ap_id,
        "target": new_ap_id.as_str(),
    });

    let activity = ActivityQueries::insert(
        &data.db,
        &NewActivity {
            ap_id: activity_ap_id.clone(),
            activity_type: "Move".to_owned(),
            actor_id: actor.id,
            object_ap_id: actor_ap_id.to_owned(),
            target_ap_id: Some(new_ap_id.as_str().to_owned()),
            object_id: None,
            ap_json,
        },
    )
    .await?;

    // Update moved_to.
    ActorQueries::set_moved_to(&data.db, actor.id, new_ap_id.as_str()).await?;

    // Enqueue delivery to followers.
    let inbox_urls = FollowQueries::list_follower_inbox_urls(&data.db, actor.id)
        .await
        .unwrap_or_default();
    if !inbox_urls.is_empty() {
        if let Err(e) = DeliveryQueries::insert_deliveries(&data.db, activity.id, &inbox_urls).await
        {
            warn!(err=%e, "move: delivery insert failed");
        }
    }

    info!(actor_id = %actor.id, new_id = %new_ap_id, "account moved");
    Ok(())
}
