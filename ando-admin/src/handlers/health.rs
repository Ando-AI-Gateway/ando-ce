use crate::server::AppState;
use axum::{extract::State, response::Json};
use serde_json::{json, Value};

pub async fn health_check(State(state): State<AppState>) -> Json<Value> {
    let stats = state.cache.stats();
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "cache": {
            "routes": stats.routes,
            "services": stats.services,
            "upstreams": stats.upstreams,
            "consumers": stats.consumers,
            "ssl_certs": stats.ssl_certs,
            "plugin_configs": stats.plugin_configs,
        },
        "plugins_loaded": state.plugin_registry.count(),
    }))
}

pub async fn schema() -> Json<Value> {
    Json(json!({
        "main": {
            "route": {
                "properties": {
                    "uri": { "type": "string" },
                    "methods": { "type": "array", "items": { "type": "string" } },
                    "host": { "type": "string" },
                    "plugins": { "type": "object" },
                    "upstream": { "type": "object" },
                    "upstream_id": { "type": "string" },
                    "service_id": { "type": "string" },
                    "priority": { "type": "integer" },
                    "enable": { "type": "boolean" },
                }
            },
            "service": {
                "properties": {
                    "name": { "type": "string" },
                    "upstream": { "type": "object" },
                    "upstream_id": { "type": "string" },
                    "plugins": { "type": "object" },
                }
            },
            "upstream": {
                "properties": {
                    "type": { "type": "string", "enum": ["roundrobin", "chash", "ewma", "least_conn"] },
                    "nodes": { "type": "object" },
                    "retries": { "type": "integer" },
                    "checks": { "type": "object" },
                }
            }
        }
    }))
}
