use crate::server::AppState;
use ando_core::route::Route;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

pub async fn list_routes(
    State(state): State<AppState>,
) -> Json<Value> {
    let routes: Vec<Route> = state.cache.routes.iter().map(|r| r.value().clone()).collect();
    Json(json!({
        "total": routes.len(),
        "list": routes
    }))
}

pub async fn get_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match state.cache.routes.get(&id) {
        Some(route) => Ok(Json(json!({ "value": route.value() }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn create_route(
    State(state): State<AppState>,
    Json(mut route): Json<Route>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    if route.id.is_empty() {
        route.id = Uuid::new_v4().to_string();
    }
    route.created_at = Some(Utc::now());
    route.updated_at = Some(Utc::now());

    let id = route.id.clone();

    // Store in etcd if available
    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let value = serde_json::to_string(&route).unwrap_or_default();
            let _ = s.put("routes", &id, &value).await;
        }
    }

    // Update cache and router
    state.cache.routes.insert(id.clone(), route.clone());
    let _ = state.router.add_route(route.clone());

    Ok((StatusCode::CREATED, Json(json!({ "value": route }))))
}

pub async fn update_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut route): Json<Route>,
) -> Result<Json<Value>, StatusCode> {
    route.id = id.clone();
    route.updated_at = Some(Utc::now());

    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let value = serde_json::to_string(&route).unwrap_or_default();
            let _ = s.put("routes", &id, &value).await;
        }
    }

    state.cache.routes.insert(id.clone(), route.clone());
    let _ = state.router.add_route(route.clone());

    Ok(Json(json!({ "value": route })))
}

pub async fn delete_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if let Ok(mut store) = state.store.try_lock() {
        if let Some(ref mut s) = *store {
            let _ = s.delete("routes", &id).await;
        }
    }

    state.cache.routes.remove(&id);
    let _ = state.router.remove_route(&id);

    Ok(StatusCode::NO_CONTENT)
}
