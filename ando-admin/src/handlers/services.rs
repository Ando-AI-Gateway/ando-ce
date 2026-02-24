use crate::persist;
use crate::server::AdminState;
use ando_core::service::Service;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{Value, json};
use std::sync::Arc;

/// PUT /apisix/admin/services/:id
pub async fn put_service(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
    Json(mut body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    body["id"] = json!(id);

    let service: Service = match serde_json::from_value(body) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": e.to_string()})),
            );
        }
    };

    state
        .cache
        .services
        .insert(service.id.clone(), service.clone());
    persist::save_state(&state);

    (
        StatusCode::OK,
        Json(json!({"id": service.id, "status": "created"})),
    )
}

/// GET /apisix/admin/services/:id
pub async fn get_service(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    match state.cache.services.get(&id) {
        Some(s) => (StatusCode::OK, Json(json!(s.value().clone()))),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Service not found"})),
        ),
    }
}

/// DELETE /apisix/admin/services/:id
pub async fn delete_service(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    state.cache.services.remove(&id);
    persist::save_state(&state);
    (StatusCode::OK, Json(json!({"deleted": true})))
}

/// GET /apisix/admin/services
pub async fn list_services(State(state): State<Arc<AdminState>>) -> Json<Value> {
    let services: Vec<Service> = state
        .cache
        .services
        .iter()
        .map(|s| s.value().clone())
        .collect();
    Json(json!({"list": services, "total": services.len()}))
}
