use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Route definition — APISIX-compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: String,

    /// URI path pattern (e.g. "/api/v1/*" or "/exact/path").
    pub uri: String,

    /// HTTP methods (empty = all methods).
    #[serde(default)]
    pub methods: Vec<String>,

    /// Host matching (optional).
    #[serde(default)]
    pub hosts: Vec<String>,

    /// Inline upstream definition.
    pub upstream: Option<crate::upstream::Upstream>,

    /// Reference to a named upstream.
    pub upstream_id: Option<String>,

    /// Reference to a named service.
    pub service_id: Option<String>,

    /// Plugins applied to this route.
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    /// Reference to a reusable plugin config set.
    pub plugin_config_id: Option<String>,

    /// Route priority (higher = matched first for same path).
    #[serde(default)]
    pub priority: i32,

    /// Route status: 1 = enabled, 0 = disabled.
    #[serde(default = "default_status")]
    pub status: u8,

    /// Human-readable name.
    pub name: Option<String>,

    /// Description.
    pub desc: Option<String>,

    /// Labels for filtering.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

fn default_status() -> u8 { 1 }

impl Route {
    /// Returns true if this route has any plugins (from route-level config).
    pub fn has_plugins(&self) -> bool {
        !self.plugins.is_empty() || self.plugin_config_id.is_some() || self.service_id.is_some()
    }

    /// Check if a given HTTP method is allowed.
    pub fn matches_method(&self, method: &str) -> bool {
        self.methods.is_empty() || self.methods.iter().any(|m| m.eq_ignore_ascii_case(method))
    }
}
