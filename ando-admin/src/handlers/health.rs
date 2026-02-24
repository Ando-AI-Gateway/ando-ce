use crate::server::AdminState;
use axum::extract::State;
use axum::response::Json;
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn health_check(State(state): State<Arc<AdminState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "engine": "ando-v2-monoio",
        "edition": state.edition
    }))
}
