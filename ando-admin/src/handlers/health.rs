use crate::server::AdminState;
use axum::extract::State;
use axum::response::Json;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn health_check(State(state): State<Arc<AdminState>>) -> Json<Value> {
    // Collect persistence metadata when a state file is configured.
    let persistence = match &state.state_file {
        None => json!({
            "mode": "none",
            "path": null,
            "file_exists": false,
            "size_bytes": null,
            "last_modified_unix": null,
        }),
        Some(path) => {
            let meta = std::fs::metadata(path).ok();
            let exists = meta.is_some();
            let size_bytes = meta.as_ref().map(|m| m.len());
            let last_modified_unix = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            json!({
                "mode": "file",
                "path": path.to_string_lossy(),
                "file_exists": exists,
                "size_bytes": size_bytes,
                "last_modified_unix": last_modified_unix,
            })
        }
    };

    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "engine": "ando-v2-monoio",
        "edition": state.edition,
        "persistence": persistence,
    }))
}
