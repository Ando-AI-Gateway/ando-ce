use ando_core::route::*;
use std::collections::HashMap;

// =============================================================================
// Helper Functions
// =============================================================================

fn make_route(id: &str, uri: &str, methods: Vec<HttpMethod>) -> Route {
    Route {
        id: id.to_string(),
        name: id.to_string(),
        description: String::new(),
        uri: uri.to_string(),
        uris: vec![],
        methods,
        host: None,
        hosts: vec![],
        remote_addrs: vec![],
        vars: vec![],
        priority: 0,
        enable: true,
        upstream: None,
        upstream_id: None,
        service_id: None,
        plugins: HashMap::new(),
        plugin_config_id: None,
        labels: HashMap::new(),
        status: 1,
        timeout: None,
        created_at: None,
        updated_at: None,
    }
}

// =============================================================================
// HttpMethod Tests
// =============================================================================

#[test]
fn test_http_method_as_str() {
    assert_eq!(HttpMethod::Get.as_str(), "GET");
    assert_eq!(HttpMethod::Post.as_str(), "POST");
    assert_eq!(HttpMethod::Put.as_str(), "PUT");
    assert_eq!(HttpMethod::Delete.as_str(), "DELETE");
    assert_eq!(HttpMethod::Patch.as_str(), "PATCH");
    assert_eq!(HttpMethod::Head.as_str(), "HEAD");
    assert_eq!(HttpMethod::Options.as_str(), "OPTIONS");
    assert_eq!(HttpMethod::Connect.as_str(), "CONNECT");
    assert_eq!(HttpMethod::Trace.as_str(), "TRACE");
}

#[test]
fn test_http_method_serialization() {
    let method = HttpMethod::Get;
    let json = serde_json::to_string(&method).unwrap();
    assert_eq!(json, "\"GET\"");

    let deserialized: HttpMethod = serde_json::from_str("\"POST\"").unwrap();
    assert_eq!(deserialized, HttpMethod::Post);
}

#[test]
fn test_http_method_equality() {
    assert_eq!(HttpMethod::Get, HttpMethod::Get);
    assert_ne!(HttpMethod::Get, HttpMethod::Post);
}

// =============================================================================
// Route method_allowed Tests
// =============================================================================

#[test]
fn test_method_allowed_empty_methods() {
    let route = make_route("r1", "/api", vec![]);
    // Empty methods means all methods allowed
    assert!(route.method_allowed("GET"));
    assert!(route.method_allowed("POST"));
    assert!(route.method_allowed("PUT"));
    assert!(route.method_allowed("DELETE"));
    assert!(route.method_allowed("PATCH"));
}

#[test]
fn test_method_allowed_specific_methods() {
    let route = make_route("r1", "/api", vec![HttpMethod::Get, HttpMethod::Post]);
    assert!(route.method_allowed("GET"));
    assert!(route.method_allowed("POST"));
    assert!(!route.method_allowed("PUT"));
    assert!(!route.method_allowed("DELETE"));
    assert!(!route.method_allowed("PATCH"));
}

#[test]
fn test_method_allowed_single_method() {
    let route = make_route("r1", "/api", vec![HttpMethod::Delete]);
    assert!(!route.method_allowed("GET"));
    assert!(!route.method_allowed("POST"));
    assert!(route.method_allowed("DELETE"));
}

// =============================================================================
// Route is_active Tests
// =============================================================================

#[test]
fn test_route_is_active_default() {
    let route = make_route("r1", "/api", vec![]);
    assert!(route.is_active());
}

#[test]
fn test_route_is_active_disabled_enable() {
    let mut route = make_route("r1", "/api", vec![]);
    route.enable = false;
    assert!(!route.is_active());
}

#[test]
fn test_route_is_active_disabled_status() {
    let mut route = make_route("r1", "/api", vec![]);
    route.status = 0;
    assert!(!route.is_active());
}

#[test]
fn test_route_is_active_both_disabled() {
    let mut route = make_route("r1", "/api", vec![]);
    route.enable = false;
    route.status = 0;
    assert!(!route.is_active());
}

// =============================================================================
// Route all_uris Tests
// =============================================================================

#[test]
fn test_all_uris_single() {
    let route = make_route("r1", "/api/v1", vec![]);
    let uris = route.all_uris();
    assert_eq!(uris, vec!["/api/v1"]);
}

#[test]
fn test_all_uris_multiple() {
    let mut route = make_route("r1", "/api/v1", vec![]);
    route.uris = vec!["/api/v2".to_string(), "/api/v3".to_string()];
    let uris = route.all_uris();
    assert_eq!(uris, vec!["/api/v1", "/api/v2", "/api/v3"]);
}

// =============================================================================
// Route all_hosts Tests
// =============================================================================

#[test]
fn test_all_hosts_none() {
    let route = make_route("r1", "/api", vec![]);
    let hosts = route.all_hosts();
    assert!(hosts.is_empty());
}

#[test]
fn test_all_hosts_single() {
    let mut route = make_route("r1", "/api", vec![]);
    route.host = Some("example.com".to_string());
    let hosts = route.all_hosts();
    assert_eq!(hosts, vec!["example.com"]);
}

#[test]
fn test_all_hosts_multiple() {
    let mut route = make_route("r1", "/api", vec![]);
    route.host = Some("example.com".to_string());
    route.hosts = vec!["other.com".to_string(), "third.com".to_string()];
    let hosts = route.all_hosts();
    assert_eq!(hosts, vec!["example.com", "other.com", "third.com"]);
}

#[test]
fn test_all_hosts_only_hosts_field() {
    let mut route = make_route("r1", "/api", vec![]);
    route.hosts = vec!["a.com".to_string(), "b.com".to_string()];
    let hosts = route.all_hosts();
    assert_eq!(hosts, vec!["a.com", "b.com"]);
}

// =============================================================================
// Route Serialization/Deserialization Tests
// =============================================================================

#[test]
fn test_route_json_roundtrip() {
    let mut route = make_route("test-1", "/api/users", vec![HttpMethod::Get, HttpMethod::Post]);
    route.priority = 10;
    route.host = Some("api.example.com".to_string());
    route.plugins.insert("key-auth".to_string(), serde_json::json!({"header": "X-API-KEY"}));

    let json = serde_json::to_string(&route).unwrap();
    let deserialized: Route = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "test-1");
    assert_eq!(deserialized.uri, "/api/users");
    assert_eq!(deserialized.methods.len(), 2);
    assert_eq!(deserialized.priority, 10);
    assert_eq!(deserialized.host, Some("api.example.com".to_string()));
    assert!(deserialized.plugins.contains_key("key-auth"));
}

#[test]
fn test_route_minimal_json_deserialization() {
    let json = r#"{"id":"r1","uri":"/test"}"#;
    let route: Route = serde_json::from_str(json).unwrap();
    assert_eq!(route.id, "r1");
    assert_eq!(route.uri, "/test");
    // Defaults
    assert!(route.enable);
    assert_eq!(route.status, 1);
    assert!(route.methods.is_empty());
    assert!(route.plugins.is_empty());
    assert!(route.is_active());
}

// =============================================================================
// InlineUpstream Tests
// =============================================================================

#[test]
fn test_inline_upstream_defaults() {
    let json = r#"{"nodes":{"127.0.0.1:8080":1}}"#;
    let upstream: InlineUpstream = serde_json::from_str(json).unwrap();
    assert_eq!(upstream.r#type, "roundrobin");
    assert_eq!(upstream.retries, 1);
    assert_eq!(upstream.pass_host, "pass");
    assert_eq!(upstream.scheme, "http");
    assert!(upstream.timeout.is_none());
    assert!(upstream.retry_timeout.is_none());
    assert!(upstream.upstream_host.is_none());
}

#[test]
fn test_inline_upstream_with_nodes() {
    let json = r#"{
        "type": "chash",
        "nodes": {"10.0.0.1:80": 5, "10.0.0.2:80": 3},
        "retries": 3,
        "scheme": "https",
        "pass_host": "rewrite",
        "upstream_host": "backend.internal"
    }"#;
    let upstream: InlineUpstream = serde_json::from_str(json).unwrap();
    assert_eq!(upstream.r#type, "chash");
    assert_eq!(upstream.nodes.len(), 2);
    assert_eq!(*upstream.nodes.get("10.0.0.1:80").unwrap(), 5);
    assert_eq!(upstream.retries, 3);
    assert_eq!(upstream.scheme, "https");
    assert_eq!(upstream.pass_host, "rewrite");
    assert_eq!(upstream.upstream_host, Some("backend.internal".to_string()));
}

// =============================================================================
// TimeoutConfig Tests
// =============================================================================

#[test]
fn test_timeout_config_defaults() {
    let json = "{}";
    let timeout: TimeoutConfig = serde_json::from_str(json).unwrap();
    assert_eq!(timeout.connect, 6.0);
    assert_eq!(timeout.send, 6.0);
    assert_eq!(timeout.read, 6.0);
}

#[test]
fn test_timeout_config_custom() {
    let json = r#"{"connect": 1.5, "send": 3.0, "read": 10.0}"#;
    let timeout: TimeoutConfig = serde_json::from_str(json).unwrap();
    assert_eq!(timeout.connect, 1.5);
    assert_eq!(timeout.send, 3.0);
    assert_eq!(timeout.read, 10.0);
}

// =============================================================================
// RouteVar Tests
// =============================================================================

#[test]
fn test_route_var_serialization() {
    let var = RouteVar {
        var: "http_x_api_version".to_string(),
        operator: "==".to_string(),
        value: serde_json::json!("v2"),
    };
    let json = serde_json::to_string(&var).unwrap();
    let deserialized: RouteVar = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.var, "http_x_api_version");
    assert_eq!(deserialized.operator, "==");
    assert_eq!(deserialized.value, serde_json::json!("v2"));
}
