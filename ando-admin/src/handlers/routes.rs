use crate::persist;
use crate::server::AdminState;
use ando_core::route::Route;
use ando_core::router::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{json, Value};
use std::sync::Arc;

/// PUT /apisix/admin/routes/:id
pub async fn put_route(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
    Json(mut body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    // Ensure the ID is set
    body["id"] = json!(id);

    let route: Route = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": e.to_string()})),
            );
        }
    };

    state.cache.routes.insert(route.id.clone(), route.clone());

    // Rebuild router
    rebuild_router(&state);

    // Persist to file (no-op if state_file is None)
    persist::save_state(&state);

    (StatusCode::OK, Json(json!({"id": route.id, "status": "created"})))
}

/// GET /apisix/admin/routes/:id
pub async fn get_route(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    match state.cache.routes.get(&id) {
        Some(r) => (StatusCode::OK, Json(json!(r.value().clone()))),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Route not found"})),
        ),
    }
}

/// DELETE /apisix/admin/routes/:id
pub async fn delete_route(
    State(state): State<Arc<AdminState>>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    state.cache.routes.remove(&id);
    rebuild_router(&state);
    persist::save_state(&state);
    (StatusCode::OK, Json(json!({"deleted": true})))
}

/// GET /apisix/admin/routes
pub async fn list_routes(
    State(state): State<Arc<AdminState>>,
) -> Json<Value> {
    let routes: Vec<Route> = state.cache.all_routes();
    Json(json!({"list": routes, "total": routes.len()}))
}

/// Rebuild the router from cache and swap it in.
fn rebuild_router(state: &AdminState) {
    let routes = state.cache.all_routes();
    let current_ver = state.router_swap.load().version();
    match Router::build(routes, current_ver + 1) {
        Ok(new_router) => {
            state.router_swap.store(Arc::new(new_router));
            state.config_changed.notify_waiters();
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to rebuild router");
        }
    }
}
