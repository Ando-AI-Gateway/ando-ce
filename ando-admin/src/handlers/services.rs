use crate::server::AppState;
use ando_core::service::Service;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

pub async fn list_services(State(state): State<AppState>) -> Json<Value> {
    let services: Vec<Service> = state.cache.services.iter().map(|r| r.value().clone()).collect();
    Json(json!({ "total": services.len(), "list": services }))
}

pub async fn get_service(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match state.cache.services.get(&id) {
        Some(svc) => Ok(Json(json!({ "value": svc.value() }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn create_service(
    State(state): State<AppState>,
    Json(mut svc): Json<Service>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    if svc.id.is_empty() {
        svc.id = Uuid::new_v4().to_string();
    }
    svc.created_at = Some(Utc::now());
    svc.updated_at = Some(Utc::now());
    let id = svc.id.clone();

    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let value = serde_json::to_string(&svc).unwrap_or_default();
            let _ = s.put("services", &id, &value).await;
        }
    }

    state.cache.services.insert(id, svc.clone());
    Ok((StatusCode::CREATED, Json(json!({ "value": svc }))))
}

pub async fn update_service(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut svc): Json<Service>,
) -> Result<Json<Value>, StatusCode> {
    svc.id = id.clone();
    svc.updated_at = Some(Utc::now());

    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let value = serde_json::to_string(&svc).unwrap_or_default();
            let _ = s.put("services", &id, &value).await;
        }
    }

    state.cache.services.insert(id, svc.clone());
    Ok(Json(json!({ "value": svc })))
}

pub async fn delete_service(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let _ = s.delete("services", &id).await;
        }
    }
    state.cache.services.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
