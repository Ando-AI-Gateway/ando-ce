use serde::{Deserialize, Serialize};

/// SSL certificate definition — APISIX-compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslCertificate {
    pub id: String,

    /// PEM-encoded certificate.
    pub cert: String,

    /// PEM-encoded private key.
    pub key: String,

    /// SNI hostnames this cert applies to.
    #[serde(default)]
    pub snis: Vec<String>,

    /// Status: 1 = enabled, 0 = disabled.
    #[serde(default = "default_status")]
    pub status: u8,
}

fn default_status() -> u8 { 1 }
