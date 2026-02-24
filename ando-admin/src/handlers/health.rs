use axum::response::Json;
use serde_json::{Value, json};

pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "engine": "ando-v2-monoio"
    }))
}
