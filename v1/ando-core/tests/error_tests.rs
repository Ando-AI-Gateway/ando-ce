use ando_core::error::AndoError;

#[test]
fn test_route_not_found_error() {
    let err = AndoError::RouteNotFound("r1".to_string());
    assert_eq!(err.to_string(), "Route not found: r1");
}

#[test]
fn test_service_not_found_error() {
    let err = AndoError::ServiceNotFound("s1".to_string());
    assert_eq!(err.to_string(), "Service not found: s1");
}

#[test]
fn test_upstream_not_found_error() {
    let err = AndoError::UpstreamNotFound("u1".to_string());
    assert_eq!(err.to_string(), "Upstream not found: u1");
}

#[test]
fn test_consumer_not_found_error() {
    let err = AndoError::ConsumerNotFound("c1".to_string());
    assert_eq!(err.to_string(), "Consumer not found: c1");
}

#[test]
fn test_plugin_not_found_error() {
    let err = AndoError::PluginNotFound("key-auth".to_string());
    assert_eq!(err.to_string(), "Plugin not found: key-auth");
}

#[test]
fn test_plugin_execution_error() {
    let err = AndoError::PluginExecution {
        plugin: "rate-limit".to_string(),
        phase: "access".to_string(),
        message: "counter overflow".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "Plugin execution error in 'rate-limit' at phase 'access': counter overflow"
    );
}

#[test]
fn test_config_error() {
    let err = AndoError::Config("invalid port".to_string());
    assert_eq!(err.to_string(), "Configuration error: invalid port");
}

#[test]
fn test_ssl_error() {
    let err = AndoError::Ssl("expired certificate".to_string());
    assert_eq!(err.to_string(), "SSL certificate error: expired certificate");
}

#[test]
fn test_store_error() {
    let err = AndoError::Store("connection refused".to_string());
    assert_eq!(err.to_string(), "etcd error: connection refused");
}

#[test]
fn test_lua_error() {
    let err = AndoError::Lua("syntax error at line 5".to_string());
    assert_eq!(err.to_string(), "Lua runtime error: syntax error at line 5");
}

#[test]
fn test_auth_failed_error() {
    let err = AndoError::AuthFailed("invalid token".to_string());
    assert_eq!(err.to_string(), "Authentication failed: invalid token");
}

#[test]
fn test_rate_limit_exceeded_error() {
    let err = AndoError::RateLimitExceeded;
    assert_eq!(err.to_string(), "Rate limit exceeded");
}

#[test]
fn test_ip_denied_error() {
    let err = AndoError::IpDenied("192.168.1.100".to_string());
    assert_eq!(err.to_string(), "IP denied: 192.168.1.100");
}

#[test]
fn test_invalid_config_error() {
    let err = AndoError::InvalidConfig("missing upstream".to_string());
    assert_eq!(err.to_string(), "Invalid configuration: missing upstream");
}

#[test]
fn test_upstream_unavailable_error() {
    let err = AndoError::UpstreamUnavailable("all nodes down".to_string());
    assert_eq!(err.to_string(), "Upstream unavailable: all nodes down");
}

#[test]
fn test_serialization_error() {
    let err = AndoError::Serialization("invalid JSON".to_string());
    assert_eq!(err.to_string(), "Serialization error: invalid JSON");
}

#[test]
fn test_internal_error() {
    let err = AndoError::Internal("unexpected panic".to_string());
    assert_eq!(err.to_string(), "Internal error: unexpected panic");
}

// =============================================================================
// Error Conversion Tests
// =============================================================================

#[test]
fn test_from_serde_json_error() {
    let result: Result<serde_json::Value, _> = serde_json::from_str("invalid json");
    let serde_err = result.unwrap_err();
    let ando_err: AndoError = serde_err.into();
    match ando_err {
        AndoError::Serialization(msg) => assert!(!msg.is_empty()),
        _ => panic!("Expected Serialization error"),
    }
}

#[test]
fn test_from_anyhow_error() {
    let anyhow_err = anyhow::anyhow!("something went wrong");
    let ando_err: AndoError = anyhow_err.into();
    match ando_err {
        AndoError::Internal(msg) => assert!(msg.contains("something went wrong")),
        _ => panic!("Expected Internal error"),
    }
}

// =============================================================================
// Error Debug Tests
// =============================================================================

#[test]
fn test_error_is_debug() {
    let err = AndoError::RouteNotFound("r1".to_string());
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("RouteNotFound"));
}
