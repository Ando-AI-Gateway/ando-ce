use crate::server::AdminState;
use axum::extract::State;
use axum::response::Json;
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn list_plugins(
    State(state): State<Arc<AdminState>>,
) -> Json<Value> {
    let plugins = state.plugin_registry.list();
    Json(json!({"plugins": plugins}))
}
