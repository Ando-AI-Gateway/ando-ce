use thiserror::Error;

/// Unified error type for Ando.
#[derive(Error, Debug)]
pub enum AndoError {
    #[error("Route not found: {0}")]
    RouteNotFound(String),

    #[error("Upstream not found: {0}")]
    UpstreamNotFound(String),

    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Consumer not found: {0}")]
    ConsumerNotFound(String),

    #[error("Plugin error: {0}")]
    PluginError(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("Auth failed: {0}")]
    AuthFailed(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("IP denied: {0}")]
    IpDenied(String),

    #[error("Store error: {0}")]
    StoreError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Internal: {0}")]
    Internal(String),
}

impl AndoError {
    /// Map to HTTP status code.
    pub fn status_code(&self) -> u16 {
        match self {
            AndoError::RouteNotFound(_) => 404,
            AndoError::UpstreamNotFound(_) => 502,
            AndoError::ServiceNotFound(_) => 503,
            AndoError::ConsumerNotFound(_) => 401,
            AndoError::AuthFailed(_) => 401,
            AndoError::RateLimited => 429,
            AndoError::IpDenied(_) => 403,
            AndoError::PluginError(_) => 500,
            _ => 500,
        }
    }

    /// JSON error body.
    pub fn to_json_body(&self) -> Vec<u8> {
        let status = self.status_code();
        let msg = self.to_string();
        format!(r#"{{"error":"{}","status":{}}}"#, msg, status).into_bytes()
    }
}
