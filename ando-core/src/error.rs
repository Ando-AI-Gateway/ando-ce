use thiserror::Error;

/// Unified error type for Ando CE.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_codes() {
        assert_eq!(AndoError::RouteNotFound("x".into()).status_code(), 404);
        assert_eq!(AndoError::UpstreamNotFound("x".into()).status_code(), 502);
        assert_eq!(AndoError::ServiceNotFound("x".into()).status_code(), 503);
        assert_eq!(AndoError::ConsumerNotFound("x".into()).status_code(), 401);
        assert_eq!(AndoError::AuthFailed("x".into()).status_code(), 401);
        assert_eq!(AndoError::RateLimited.status_code(), 429);
        assert_eq!(AndoError::IpDenied("x".into()).status_code(), 403);
        assert_eq!(AndoError::PluginError("x".into()).status_code(), 500);
        assert_eq!(AndoError::Internal("x".into()).status_code(), 500);
    }

    #[test]
    fn test_json_body_is_valid_json() {
        let err = AndoError::AuthFailed("bad key".into());
        let body = err.to_json_body();
        let parsed: serde_json::Value =
            serde_json::from_slice(&body).expect("to_json_body must produce valid JSON");
        assert_eq!(parsed["status"], 401);
        assert!(parsed["error"].as_str().is_some());
    }

    #[test]
    fn test_json_body_contains_status_and_message() {
        let err = AndoError::RouteNotFound("r1".into());
        let text = String::from_utf8(err.to_json_body()).unwrap();
        assert!(text.contains("404"), "body must contain status code");
        assert!(text.contains("r1"), "body must contain the route id");
    }

    #[test]
    fn test_rate_limited_body() {
        let err = AndoError::RateLimited;
        let parsed: serde_json::Value =
            serde_json::from_slice(&err.to_json_body()).unwrap();
        assert_eq!(parsed["status"], 429);
    }

    #[test]
    fn test_display_messages() {
        assert_eq!(AndoError::AuthFailed("invalid key".into()).to_string(), "Auth failed: invalid key");
        assert_eq!(AndoError::RouteNotFound("route1".into()).to_string(), "Route not found: route1");
        assert_eq!(AndoError::RateLimited.to_string(), "Rate limited");
        assert_eq!(AndoError::IpDenied("1.2.3.4".into()).to_string(), "IP denied: 1.2.3.4");
        assert_eq!(AndoError::UpstreamNotFound("us1".into()).to_string(), "Upstream not found: us1");
    }

    #[test]
    fn test_ip_denied_is_403() {
        let err = AndoError::IpDenied("192.168.1.1".into());
        assert_eq!(err.status_code(), 403);
        let body = String::from_utf8(err.to_json_body()).unwrap();
        assert!(body.contains("403"));
        assert!(body.contains("192.168.1.1"));
    }
}
