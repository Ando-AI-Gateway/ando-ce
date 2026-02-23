use crate::persist;
use crate::server::AdminState;
use ando_core::consumer::Consumer;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn put_consumer(
    State(state): State<Arc<AdminState>>,
    Path(username): Path<String>,
    Json(mut body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    body["username"] = json!(username);

    let consumer: Consumer = match serde_json::from_value(body) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})));
        }
    };

    state
        .cache
        .consumers
        .insert(consumer.username.clone(), consumer.clone());
    state.cache.rebuild_consumer_key_index();
    persist::save_state(&state);

    (
        StatusCode::OK,
        Json(json!({"username": consumer.username, "status": "created"})),
    )
}

pub async fn get_consumer(
    State(state): State<Arc<AdminState>>,
    Path(username): Path<String>,
) -> (StatusCode, Json<Value>) {
    match state.cache.consumers.get(&username) {
        Some(c) => (StatusCode::OK, Json(json!(c.value().clone()))),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Consumer not found"})),
        ),
    }
}

pub async fn delete_consumer(
    State(state): State<Arc<AdminState>>,
    Path(username): Path<String>,
) -> (StatusCode, Json<Value>) {
    state.cache.consumers.remove(&username);
    state.cache.rebuild_consumer_key_index();
    persist::save_state(&state);
    (StatusCode::OK, Json(json!({"deleted": true})))
}

pub async fn list_consumers(
    State(state): State<Arc<AdminState>>,
) -> Json<Value> {
    let consumers: Vec<Consumer> = state
        .cache
        .consumers
        .iter()
        .map(|c| c.value().clone())
        .collect();
    Json(json!({"list": consumers, "total": consumers.len()}))
}
