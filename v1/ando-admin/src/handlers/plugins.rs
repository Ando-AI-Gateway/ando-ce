use crate::server::AppState;
use axum::{extract::State, response::Json};
use serde_json::{json, Value};

pub async fn list_plugins(State(state): State<AppState>) -> Json<Value> {
    let plugins = state.plugin_registry.list();
    Json(json!({
        "total": plugins.len(),
        "list": plugins
    }))
}
