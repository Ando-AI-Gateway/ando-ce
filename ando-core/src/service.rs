use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Service definition — APISIX-compatible.
/// A service is a reusable bundle of upstream + plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    pub name: Option<String>,
    pub desc: Option<String>,

    /// Upstream reference.
    pub upstream_id: Option<String>,

    /// Inline upstream.
    pub upstream: Option<crate::upstream::Upstream>,

    /// Plugins applied to routes using this service.
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}
