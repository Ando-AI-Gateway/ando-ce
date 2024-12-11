use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SSL certificate configuration for dynamic TLS/SNI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslCert {
    /// Unique identifier
    pub id: String,

    /// SNI hostnames this cert covers
    pub snis: Vec<String>,

    /// PEM-encoded certificate chain
    pub cert: String,

    /// PEM-encoded private key
    pub key: String,

    /// Client CA certificate (for mTLS)
    #[serde(default)]
    pub client_cert: Option<String>,

    /// Whether this cert is enabled
    #[serde(default = "default_true")]
    pub status: bool,

    /// Certificate expiration
    #[serde(default)]
    pub validity_end: Option<chrono::DateTime<chrono::Utc>>,

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
