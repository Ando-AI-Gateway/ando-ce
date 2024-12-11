use crate::server::AppState;
use ando_core::consumer::Consumer;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde_json::{json, Value};

pub async fn list_consumers(State(state): State<AppState>) -> Json<Value> {
    let consumers: Vec<Consumer> = state.cache.consumers.iter().map(|r| r.value().clone()).collect();
    Json(json!({ "total": consumers.len(), "list": consumers }))
}

pub async fn get_consumer(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match state.cache.consumers.get(&id) {
        Some(c) => Ok(Json(json!({ "value": c.value() }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn create_consumer(
    State(state): State<AppState>,
    Json(mut consumer): Json<Consumer>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    if consumer.id.is_empty() {
        consumer.id = consumer.username.clone();
    }
    consumer.created_at = Some(Utc::now());
    consumer.updated_at = Some(Utc::now());
    let id = consumer.id.clone();

    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let value = serde_json::to_string(&consumer).unwrap_or_default();
            let _ = s.put("consumers", &id, &value).await;
        }
    }

    state.cache.consumers.insert(id, consumer.clone());
    Ok((StatusCode::CREATED, Json(json!({ "value": consumer }))))
}

pub async fn upsert_consumer(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut consumer): Json<Consumer>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    // Force the username/id from the path
    if consumer.username.is_empty() {
        consumer.username = id.clone();
    }
    consumer.id = id.clone();
    consumer.updated_at = Some(Utc::now());
    if consumer.created_at.is_none() {
        consumer.created_at = Some(Utc::now());
    }

    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let value = serde_json::to_string(&consumer).unwrap_or_default();
            let _ = s.put("consumers", &id, &value).await;
        }
    }

    let existed = state.cache.consumers.contains_key(&id);
    state.cache.consumers.insert(id, consumer.clone());
    let status = if existed { StatusCode::OK } else { StatusCode::CREATED };
    Ok((status, Json(json!({ "value": consumer }))))
}

pub async fn delete_consumer(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let _ = s.delete("consumers", &id).await;
        }
    }
    state.cache.consumers.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
