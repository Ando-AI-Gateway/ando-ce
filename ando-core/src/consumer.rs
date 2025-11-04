use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Consumer definition — APISIX-compatible.
/// Represents an API consumer with authentication credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consumer {
    pub username: String,

    /// Plugins with consumer-specific config (e.g. key-auth key).
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    pub desc: Option<String>,

    #[serde(default)]
    pub labels: HashMap<String, String>,
}
