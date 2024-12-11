use thiserror::Error;

/// Core error types for the Ando API Gateway.
#[derive(Debug, Error)]
pub enum AndoError {
    #[error("Route not found: {0}")]
    RouteNotFound(String),

    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Upstream not found: {0}")]
    UpstreamNotFound(String),

    #[error("Consumer not found: {0}")]
    ConsumerNotFound(String),

    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("Plugin execution error in '{plugin}' at phase '{phase}': {message}")]
    PluginExecution {
        plugin: String,
        phase: String,
        message: String,
    },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("SSL certificate error: {0}")]
    Ssl(String),

    #[error("etcd error: {0}")]
    Store(String),

    #[error("Lua runtime error: {0}")]
    Lua(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("IP denied: {0}")]
    IpDenied(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Upstream unavailable: {0}")]
    UpstreamUnavailable(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for AndoError {
    fn from(err: serde_json::Error) -> Self {
        AndoError::Serialization(err.to_string())
    }
}

impl From<anyhow::Error> for AndoError {
    fn from(err: anyhow::Error) -> Self {
        AndoError::Internal(err.to_string())
    }
}
