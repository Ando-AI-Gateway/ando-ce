use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A Consumer represents an API user/client with associated credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consumer {
    /// Unique identifier (typically the consumer name)
    #[serde(default)]
    pub id: String,

    /// Username
    #[serde(default)]
    pub username: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Plugin configurations (credentials per plugin)
    /// e.g., "key-auth" -> {"key": "abc123"}
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    /// Consumer group
    #[serde(default)]
    pub group: Option<String>,

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
