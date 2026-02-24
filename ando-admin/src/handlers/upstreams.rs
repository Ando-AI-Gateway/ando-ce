use crate::persist;
use crate::server::AdminState;
use ando_core::upstream::Upstream;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn put_upstream(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
    Json(mut body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    body["id"] = json!(id);

    let upstream: Upstream = match serde_json::from_value(body) {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": e.to_string()})),
            );
        }
    };

    let uid = upstream.id.clone().unwrap_or(id.clone());
    state.cache.upstreams.insert(uid.clone(), upstream);
    persist::save_state(&state);

    (
        StatusCode::OK,
        Json(json!({"id": uid, "status": "created"})),
    )
}

pub async fn get_upstream(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    match state.cache.upstreams.get(&id) {
        Some(u) => (StatusCode::OK, Json(json!(u.value().clone()))),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Upstream not found"})),
        ),
    }
}

pub async fn delete_upstream(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    state.cache.upstreams.remove(&id);
    persist::save_state(&state);
    (StatusCode::OK, Json(json!({"deleted": true})))
}

pub async fn list_upstreams(State(state): State<Arc<AdminState>>) -> Json<Value> {
    let upstreams: Vec<Upstream> = state
        .cache
        .upstreams
        .iter()
        .map(|u| u.value().clone())
        .collect();
    Json(json!({"list": upstreams, "total": upstreams.len()}))
}
