use crate::server::AppState;
use ando_core::upstream::Upstream;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

pub async fn list_upstreams(State(state): State<AppState>) -> Json<Value> {
    let upstreams: Vec<Upstream> = state.cache.upstreams.iter().map(|r| r.value().clone()).collect();
    Json(json!({ "total": upstreams.len(), "list": upstreams }))
}

pub async fn get_upstream(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match state.cache.upstreams.get(&id) {
        Some(u) => Ok(Json(json!({ "value": u.value() }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn create_upstream(
    State(state): State<AppState>,
    Json(mut upstream): Json<Upstream>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    if upstream.id.is_empty() {
        upstream.id = Uuid::new_v4().to_string();
    }
    upstream.created_at = Some(Utc::now());
    upstream.updated_at = Some(Utc::now());
    let id = upstream.id.clone();

    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let value = serde_json::to_string(&upstream).unwrap_or_default();
            let _ = s.put("upstreams", &id, &value).await;
        }
    }

    state.cache.upstreams.insert(id, upstream.clone());
    Ok((StatusCode::CREATED, Json(json!({ "value": upstream }))))
}

pub async fn update_upstream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut upstream): Json<Upstream>,
) -> Result<Json<Value>, StatusCode> {
    upstream.id = id.clone();
    upstream.updated_at = Some(Utc::now());

    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let value = serde_json::to_string(&upstream).unwrap_or_default();
            let _ = s.put("upstreams", &id, &value).await;
        }
    }

    state.cache.upstreams.insert(id, upstream.clone());
    Ok(Json(json!({ "value": upstream })))
}

pub async fn delete_upstream(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let _ = s.delete("upstreams", &id).await;
        }
    }
    state.cache.upstreams.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
