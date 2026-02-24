use crate::server::AdminState;
use axum::extract::State;
use axum::response::Json;
use serde_json::{Value, json};
use std::sync::Arc;

/// Community Edition plugins — loaded into the registry at startup.
const CE_PLUGINS: &[(&str, &str, bool)] = &[
    ("key-auth", "Access", true),
    ("jwt-auth", "Access", true),
    ("basic-auth", "Access", true),
    ("ip-restriction", "Access", true),
    ("rate-limiting", "Access", true),
    ("cors", "HeaderFilter", true),
];

/// Enterprise Edition plugins — visible in the API but not available in CE.
const EE_PLUGINS: &[(&str, &str, bool)] = &[
    ("hmac-auth", "Access", false),
    ("oauth2", "Access", false),
    ("rate-limiting-advanced", "Access", false),
    ("traffic-mirror", "Upstream", false),
    ("canary-release", "Upstream", false),
    ("circuit-breaker", "Upstream", false),
];

pub async fn list_plugins(State(state): State<Arc<AdminState>>) -> Json<Value> {
    // CE plugins: names from the live registry
    let registered = state.plugin_registry.list();

    let ce: Vec<Value> = CE_PLUGINS
        .iter()
        .map(|(name, phase, _)| {
            json!({
                "name":      name,
                "phase":     phase,
                "available": registered.contains(name),
                "edition":   "community"
            })
        })
        .collect();

    let ee: Vec<Value> = EE_PLUGINS
        .iter()
        .map(|(name, phase, _)| {
            json!({
                "name":      name,
                "phase":     phase,
                "available": false,
                "edition":   "enterprise",
                "locked":    true,
                "upgrade_url": "https://andolabs.org/enterprise"
            })
        })
        .collect();

    Json(json!({
        "plugins": ce,
        "enterprise_plugins": ee,
        "edition": "community"
    }))
}
