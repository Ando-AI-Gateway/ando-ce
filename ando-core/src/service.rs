use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A Service is a reusable set of plugins and upstream configuration
/// that can be referenced by multiple routes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    /// Unique identifier
    pub id: String,

    /// Human-readable name
    #[serde(default)]
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Inline upstream
    #[serde(default)]
    pub upstream: Option<crate::route::InlineUpstream>,

    /// Reference to a named upstream
    #[serde(default)]
    pub upstream_id: Option<String>,

    /// Plugin configuration: plugin_name -> config
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    /// Whether this service is enabled
    #[serde(default = "default_true")]
    pub enable: bool,

    /// Labels
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Creation timestamp
    #[serde(default)]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Update timestamp
    #[serde(default)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn default_true() -> bool {
    true
}
