use ando_plugin::plugin::{Phase, PluginContext, PluginResult};
use std::collections::HashMap;

// =============================================================================
// Phase Tests
// =============================================================================

#[test]
fn test_phase_as_str() {
    assert_eq!(Phase::Rewrite.as_str(), "rewrite");
    assert_eq!(Phase::Access.as_str(), "access");
    assert_eq!(Phase::BeforeProxy.as_str(), "before_proxy");
    assert_eq!(Phase::HeaderFilter.as_str(), "header_filter");
    assert_eq!(Phase::BodyFilter.as_str(), "body_filter");
    assert_eq!(Phase::Log.as_str(), "log");
}

#[test]
fn test_phase_display() {
    assert_eq!(format!("{}", Phase::Rewrite), "rewrite");
    assert_eq!(format!("{}", Phase::Access), "access");
    assert_eq!(format!("{}", Phase::BeforeProxy), "before_proxy");
    assert_eq!(format!("{}", Phase::HeaderFilter), "header_filter");
    assert_eq!(format!("{}", Phase::BodyFilter), "body_filter");
    assert_eq!(format!("{}", Phase::Log), "log");
}

#[test]
fn test_phase_all() {
    let all = Phase::all();
    assert_eq!(all.len(), 6);
    assert_eq!(all[0], Phase::Rewrite);
    assert_eq!(all[1], Phase::Access);
    assert_eq!(all[2], Phase::BeforeProxy);
    assert_eq!(all[3], Phase::HeaderFilter);
    assert_eq!(all[4], Phase::BodyFilter);
    assert_eq!(all[5], Phase::Log);
}

#[test]
fn test_phase_ordering() {
    assert!(Phase::Rewrite < Phase::Access);
    assert!(Phase::Access < Phase::BeforeProxy);
    assert!(Phase::BeforeProxy < Phase::HeaderFilter);
    assert!(Phase::HeaderFilter < Phase::BodyFilter);
    assert!(Phase::BodyFilter < Phase::Log);
}

#[test]
fn test_phase_equality() {
    assert_eq!(Phase::Access, Phase::Access);
    assert_ne!(Phase::Access, Phase::Rewrite);
}

#[test]
fn test_phase_clone_and_copy() {
    let phase = Phase::Access;
    let cloned = phase.clone();
    let copied = phase;
    assert_eq!(phase, cloned);
    assert_eq!(phase, copied);
}

// =============================================================================
// PluginContext Tests
// =============================================================================

#[test]
fn test_plugin_context_new() {
    let headers = HashMap::from([
        ("content-type".to_string(), "application/json".to_string()),
        ("host".to_string(), "example.com".to_string()),
    ]);

    let ctx = PluginContext::new(
        "GET".to_string(),
        "/api/users?page=1".to_string(),
        headers,
        "192.168.1.1".to_string(),
        "route-1".to_string(),
    );

    assert_eq!(ctx.request_method, "GET");
    assert_eq!(ctx.request_uri, "/api/users?page=1");
    assert_eq!(ctx.request_path, "/api/users");
    assert_eq!(ctx.request_query, "page=1");
    assert_eq!(ctx.client_ip, "192.168.1.1");
    assert_eq!(ctx.route_id, "route-1");
    assert!(ctx.request_body.is_none());
    assert!(ctx.response_status.is_none());
    assert!(ctx.consumer.is_none());
    assert!(ctx.service_id.is_none());
    assert!(ctx.upstream_addr.is_none());
}

#[test]
fn test_plugin_context_no_query() {
    let ctx = PluginContext::new(
        "POST".to_string(),
        "/api/users".to_string(),
        HashMap::new(),
        "10.0.0.1".to_string(),
        "r1".to_string(),
    );

    assert_eq!(ctx.request_path, "/api/users");
    assert_eq!(ctx.request_query, "");
}

#[test]
fn test_plugin_context_get_header() {
    let headers = HashMap::from([
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Authorization".to_string(), "Bearer token123".to_string()),
    ]);

    let ctx = PluginContext::new(
        "GET".to_string(),
        "/api".to_string(),
        headers,
        "127.0.0.1".to_string(),
        "r1".to_string(),
    );

    // Case-insensitive lookup
    assert_eq!(ctx.get_header("content-type"), Some("application/json"));
    assert_eq!(ctx.get_header("Content-Type"), Some("application/json"));
    assert_eq!(ctx.get_header("CONTENT-TYPE"), Some("application/json"));
    assert_eq!(ctx.get_header("authorization"), Some("Bearer token123"));
    assert!(ctx.get_header("x-missing").is_none());
}

#[test]
fn test_plugin_context_set_header() {
    let mut ctx = PluginContext::new(
        "GET".to_string(),
        "/api".to_string(),
        HashMap::new(),
        "127.0.0.1".to_string(),
        "r1".to_string(),
    );

    ctx.set_header("X-Custom".to_string(), "value1".to_string());
    assert!(ctx.request_headers.contains_key("X-Custom"));
    assert_eq!(ctx.request_headers.get("X-Custom").unwrap(), "value1");
}

#[test]
fn test_plugin_context_remove_header() {
    let headers = HashMap::from([
        ("content-type".to_string(), "application/json".to_string()),
        ("authorization".to_string(), "Bearer token".to_string()),
        ("x-custom".to_string(), "value".to_string()),
    ]);

    let mut ctx = PluginContext::new(
        "GET".to_string(),
        "/api".to_string(),
        headers,
        "127.0.0.1".to_string(),
        "r1".to_string(),
    );

    assert_eq!(ctx.request_headers.len(), 3);
    ctx.remove_header("authorization");
    assert_eq!(ctx.request_headers.len(), 2);
    assert!(ctx.get_header("authorization").is_none());

    // Case-insensitive removal
    ctx.remove_header("Content-Type");
    assert_eq!(ctx.request_headers.len(), 1);
    assert!(ctx.get_header("content-type").is_none());
}

#[test]
fn test_plugin_context_set_response_header() {
    let mut ctx = PluginContext::new(
        "GET".to_string(),
        "/api".to_string(),
        HashMap::new(),
        "127.0.0.1".to_string(),
        "r1".to_string(),
    );

    ctx.set_response_header("X-RateLimit-Remaining".to_string(), "99".to_string());
    assert_eq!(
        ctx.response_headers.get("X-RateLimit-Remaining").unwrap(),
        "99"
    );
}

#[test]
fn test_plugin_context_elapsed_ms() {
    let ctx = PluginContext::new(
        "GET".to_string(),
        "/api".to_string(),
        HashMap::new(),
        "127.0.0.1".to_string(),
        "r1".to_string(),
    );

    // Should be very small since we just created it
    let elapsed = ctx.elapsed_ms();
    assert!(elapsed >= 0.0);
    assert!(elapsed < 100.0); // Should be well under 100ms
}

#[test]
fn test_plugin_context_vars() {
    let mut ctx = PluginContext::new(
        "GET".to_string(),
        "/api".to_string(),
        HashMap::new(),
        "127.0.0.1".to_string(),
        "r1".to_string(),
    );

    // Initially empty
    assert!(ctx.get_var("api_key").is_none());

    // Set a var
    ctx.set_var("api_key".to_string(), serde_json::json!("abc123"));
    assert_eq!(
        ctx.get_var("api_key"),
        Some(&serde_json::json!("abc123"))
    );

    // Set another var
    ctx.set_var("jwt_sub".to_string(), serde_json::json!("user-1"));
    assert_eq!(
        ctx.get_var("jwt_sub"),
        Some(&serde_json::json!("user-1"))
    );

    // Overwrite
    ctx.set_var("api_key".to_string(), serde_json::json!("xyz789"));
    assert_eq!(
        ctx.get_var("api_key"),
        Some(&serde_json::json!("xyz789"))
    );
}

// =============================================================================
// PluginResult Tests
// =============================================================================

#[test]
fn test_plugin_result_continue_debug() {
    let result = PluginResult::Continue;
    let debug = format!("{:?}", result);
    assert!(debug.contains("Continue"));
}

#[test]
fn test_plugin_result_response_debug() {
    let result = PluginResult::Response {
        status: 429,
        headers: HashMap::from([("content-type".to_string(), "application/json".to_string())]),
        body: Some(b"rate limited".to_vec()),
    };
    let debug = format!("{:?}", result);
    assert!(debug.contains("Response"));
    assert!(debug.contains("429"));
}

#[test]
fn test_plugin_result_error_debug() {
    let result = PluginResult::Error("something went wrong".to_string());
    let debug = format!("{:?}", result);
    assert!(debug.contains("Error"));
    assert!(debug.contains("something went wrong"));
}

// =============================================================================
// PluginInstance Tests
// =============================================================================

#[test]
fn test_plugin_context_path_params() {
    let mut ctx = PluginContext::new(
        "GET".to_string(),
        "/api/users/123".to_string(),
        HashMap::new(),
        "127.0.0.1".to_string(),
        "r1".to_string(),
    );

    ctx.path_params.insert("id".to_string(), "123".to_string());
    assert_eq!(ctx.path_params.get("id").unwrap(), "123");
}

#[test]
fn test_plugin_context_complex_query() {
    let ctx = PluginContext::new(
        "GET".to_string(),
        "/api/search?q=rust&page=2&limit=10".to_string(),
        HashMap::new(),
        "127.0.0.1".to_string(),
        "r1".to_string(),
    );

    assert_eq!(ctx.request_path, "/api/search");
    assert_eq!(ctx.request_query, "q=rust&page=2&limit=10");
}
