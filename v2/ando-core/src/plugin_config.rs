use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Reusable plugin config set — APISIX-compatible.
/// Can be referenced by multiple routes to share plugin configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub id: String,
    pub desc: Option<String>,

    /// Plugin configurations.
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    #[serde(default)]
    pub labels: HashMap<String, String>,
}
