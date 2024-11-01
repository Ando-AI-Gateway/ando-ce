use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A PluginConfig is a reusable plugin configuration set
/// that can be referenced by multiple routes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Unique identifier
    pub id: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Plugin configurations: plugin_name -> config
    pub plugins: HashMap<String, serde_json::Value>,

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
