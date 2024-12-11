use crate::server::AppState;
use ando_core::ssl::SslCert;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

pub async fn list_ssl(State(state): State<AppState>) -> Json<Value> {
    let certs: Vec<SslCert> = state.cache.ssl_certs.iter().map(|r| r.value().clone()).collect();
    Json(json!({ "total": certs.len(), "list": certs }))
}

pub async fn get_ssl(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match state.cache.ssl_certs.get(&id) {
        Some(c) => Ok(Json(json!({ "value": c.value() }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn create_ssl(
    State(state): State<AppState>,
    Json(mut cert): Json<SslCert>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    if cert.id.is_empty() {
        cert.id = Uuid::new_v4().to_string();
    }
    cert.created_at = Some(Utc::now());
    cert.updated_at = Some(Utc::now());
    let id = cert.id.clone();

    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let value = serde_json::to_string(&cert).unwrap_or_default();
            let _ = s.put("ssl", &id, &value).await;
        }
    }

    state.cache.ssl_certs.insert(id, cert.clone());
    Ok((StatusCode::CREATED, Json(json!({ "value": cert }))))
}

pub async fn delete_ssl(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let _ = s.delete("ssl", &id).await;
        }
    }
    state.cache.ssl_certs.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
