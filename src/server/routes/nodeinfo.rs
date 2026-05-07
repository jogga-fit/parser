use activitypub_federation::config::Data;
use axum::{Json, response::IntoResponse};
use serde_json::json;

use crate::server::state::AppState;

pub async fn well_known(data: Data<AppState>) -> impl IntoResponse {
    let domain = &data.config.instance.domain;
    let scheme = data.config.instance.scheme();
    Json(json!({
        "links": [{
            "rel": "http://nodeinfo.diaspora.software/ns/schema/2.1",
            "href": format!("{scheme}://{domain}/nodeinfo/2.1")
        }]
    }))
}

pub async fn nodeinfo(data: Data<AppState>) -> impl IntoResponse {
    Json(json!({
        "version": "2.1",
        "software": {
            "name": "jogga",
            "version": env!("CARGO_PKG_VERSION"),
            "repository": "https://github.com/jogga-fit/core"
        },
        "protocols": ["activitypub"],
        "usage": {
            "users": {
                "total": 1,
                "activeMonth": 1,
                "activeHalfyear": 1
            },
            "localPosts": 0
        },
        "openRegistrations": false,
        "metadata": {
            "nodeName": data.config.instance.name,
            "nodeDescription": data.config.instance.description
        }
    }))
}
