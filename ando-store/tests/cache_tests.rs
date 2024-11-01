use ando_store::cache::ConfigCache;

// =============================================================================
// ConfigCache Basic Tests
// =============================================================================

#[test]
fn test_cache_new() {
    let cache = ConfigCache::new();
    let stats = cache.stats();
    assert_eq!(stats.routes, 0);
    assert_eq!(stats.services, 0);
    assert_eq!(stats.upstreams, 0);
    assert_eq!(stats.consumers, 0);
    assert_eq!(stats.ssl_certs, 0);
    assert_eq!(stats.plugin_configs, 0);
}

#[test]
fn test_cache_default() {
    let cache = ConfigCache::default();
    assert_eq!(cache.stats().routes, 0);
}

// =============================================================================
// Route Cache Tests
// =============================================================================

#[test]
fn test_cache_apply_route_put() {
    let cache = ConfigCache::new();
    let route_json = r#"{"id":"r1","uri":"/api/users","name":"users","description":"","uris":[],"methods":[],"hosts":[],"remote_addrs":[],"vars":[],"priority":0,"enable":true,"plugins":{},"labels":{},"status":1}"#;

    cache.apply_change("routes", "r1", Some(route_json));
    assert_eq!(cache.stats().routes, 1);
    assert!(cache.routes.get("r1").is_some());
}

#[test]
fn test_cache_apply_route_delete() {
    let cache = ConfigCache::new();
    let route_json = r#"{"id":"r1","uri":"/api/users","name":"","description":"","uris":[],"methods":[],"hosts":[],"remote_addrs":[],"vars":[],"priority":0,"enable":true,"plugins":{},"labels":{},"status":1}"#;

    cache.apply_change("routes", "r1", Some(route_json));
    assert_eq!(cache.stats().routes, 1);

    cache.apply_change("routes", "r1", None);
    assert_eq!(cache.stats().routes, 0);
}

#[test]
fn test_cache_apply_route_update() {
    let cache = ConfigCache::new();
    let route_json_v1 = r#"{"id":"r1","uri":"/api/v1","name":"v1","description":"","uris":[],"methods":[],"hosts":[],"remote_addrs":[],"vars":[],"priority":0,"enable":true,"plugins":{},"labels":{},"status":1}"#;
    let route_json_v2 = r#"{"id":"r1","uri":"/api/v2","name":"v2","description":"","uris":[],"methods":[],"hosts":[],"remote_addrs":[],"vars":[],"priority":0,"enable":true,"plugins":{},"labels":{},"status":1}"#;

    cache.apply_change("routes", "r1", Some(route_json_v1));
    assert_eq!(cache.routes.get("r1").unwrap().uri, "/api/v1");

    cache.apply_change("routes", "r1", Some(route_json_v2));
    assert_eq!(cache.routes.get("r1").unwrap().uri, "/api/v2");
    assert_eq!(cache.stats().routes, 1);
}

#[test]
fn test_cache_apply_route_invalid_json() {
    let cache = ConfigCache::new();
    cache.apply_change("routes", "r1", Some("not valid json"));
    // Invalid JSON should be silently ignored
    assert_eq!(cache.stats().routes, 0);
}

// =============================================================================
// Service Cache Tests
// =============================================================================

#[test]
fn test_cache_apply_service() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"s1","name":"svc","enable":true}"#;
    cache.apply_change("services", "s1", Some(json));
    assert_eq!(cache.stats().services, 1);
}

#[test]
fn test_cache_apply_service_delete() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"s1","name":"svc","enable":true}"#;
    cache.apply_change("services", "s1", Some(json));
    cache.apply_change("services", "s1", None);
    assert_eq!(cache.stats().services, 0);
}

// =============================================================================
// Upstream Cache Tests
// =============================================================================

#[test]
fn test_cache_apply_upstream() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"u1","nodes":{"127.0.0.1:8080":1}}"#;
    cache.apply_change("upstreams", "u1", Some(json));
    assert_eq!(cache.stats().upstreams, 1);
}

#[test]
fn test_cache_apply_upstream_delete() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"u1","nodes":{"127.0.0.1:8080":1}}"#;
    cache.apply_change("upstreams", "u1", Some(json));
    cache.apply_change("upstreams", "u1", None);
    assert_eq!(cache.stats().upstreams, 0);
}

// =============================================================================
// Consumer Cache Tests
// =============================================================================

#[test]
fn test_cache_apply_consumer() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"c1","username":"user1"}"#;
    cache.apply_change("consumers", "c1", Some(json));
    assert_eq!(cache.stats().consumers, 1);
    let consumer = cache.consumers.get("c1").unwrap();
    assert_eq!(consumer.username, "user1");
}

#[test]
fn test_cache_apply_consumer_delete() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"c1","username":"user1"}"#;
    cache.apply_change("consumers", "c1", Some(json));
    cache.apply_change("consumers", "c1", None);
    assert_eq!(cache.stats().consumers, 0);
}

// =============================================================================
// SSL Cache Tests
// =============================================================================

#[test]
fn test_cache_apply_ssl() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"ssl1","snis":["example.com"],"cert":"cert","key":"key"}"#;
    cache.apply_change("ssl", "ssl1", Some(json));
    assert_eq!(cache.stats().ssl_certs, 1);
}

#[test]
fn test_cache_apply_ssl_delete() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"ssl1","snis":["example.com"],"cert":"cert","key":"key"}"#;
    cache.apply_change("ssl", "ssl1", Some(json));
    cache.apply_change("ssl", "ssl1", None);
    assert_eq!(cache.stats().ssl_certs, 0);
}

// =============================================================================
// PluginConfig Cache Tests
// =============================================================================

#[test]
fn test_cache_apply_plugin_config() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"pc1","plugins":{"key-auth":{"header":"X-API-KEY"}}}"#;
    cache.apply_change("plugin_configs", "pc1", Some(json));
    assert_eq!(cache.stats().plugin_configs, 1);
}

#[test]
fn test_cache_apply_plugin_config_delete() {
    let cache = ConfigCache::new();
    let json = r#"{"id":"pc1","plugins":{"key-auth":{"header":"X-API-KEY"}}}"#;
    cache.apply_change("plugin_configs", "pc1", Some(json));
    cache.apply_change("plugin_configs", "pc1", None);
    assert_eq!(cache.stats().plugin_configs, 0);
}

// =============================================================================
// Unknown Resource Type Tests
// =============================================================================

#[test]
fn test_cache_apply_unknown_resource_type() {
    let cache = ConfigCache::new();
    // Should not panic, just log a warning
    cache.apply_change("unknown", "id1", Some("{}"));
    // All counts should remain 0
    let stats = cache.stats();
    assert_eq!(stats.routes, 0);
    assert_eq!(stats.services, 0);
}

// =============================================================================
// Cache Stats Display Tests
// =============================================================================

#[test]
fn test_cache_stats_display() {
    let cache = ConfigCache::new();
    let stats = cache.stats();
    let display = format!("{}", stats);
    assert!(display.contains("routes=0"));
    assert!(display.contains("services=0"));
    assert!(display.contains("upstreams=0"));
    assert!(display.contains("consumers=0"));
    assert!(display.contains("ssl=0"));
    assert!(display.contains("plugin_configs=0"));
}

#[test]
fn test_cache_stats_debug() {
    let cache = ConfigCache::new();
    let stats = cache.stats();
    let debug = format!("{:?}", stats);
    assert!(debug.contains("CacheStats"));
}

// =============================================================================
// Cache Clone Tests
// =============================================================================

#[test]
fn test_cache_clone_shares_data() {
    let cache = ConfigCache::new();
    let cloned = cache.clone();

    let json = r#"{"id":"r1","uri":"/api","name":"","description":"","uris":[],"methods":[],"hosts":[],"remote_addrs":[],"vars":[],"priority":0,"enable":true,"plugins":{},"labels":{},"status":1}"#;
    cache.apply_change("routes", "r1", Some(json));

    // Clone should see the same data (Arc-based sharing)
    assert_eq!(cloned.stats().routes, 1);
}

// =============================================================================
// Multiple Resources Tests
// =============================================================================

#[test]
fn test_cache_multiple_resources() {
    let cache = ConfigCache::new();

    let route_json = r#"{"id":"r1","uri":"/api","name":"","description":"","uris":[],"methods":[],"hosts":[],"remote_addrs":[],"vars":[],"priority":0,"enable":true,"plugins":{},"labels":{},"status":1}"#;
    let service_json = r#"{"id":"s1","name":"svc","enable":true}"#;
    let upstream_json = r#"{"id":"u1","nodes":{"localhost:80":1}}"#;
    let consumer_json = r#"{"id":"c1","username":"user"}"#;
    let ssl_json = r#"{"id":"ssl1","snis":["test.com"],"cert":"c","key":"k"}"#;
    let pc_json = r#"{"id":"pc1","plugins":{"test":{}}}"#;

    cache.apply_change("routes", "r1", Some(route_json));
    cache.apply_change("services", "s1", Some(service_json));
    cache.apply_change("upstreams", "u1", Some(upstream_json));
    cache.apply_change("consumers", "c1", Some(consumer_json));
    cache.apply_change("ssl", "ssl1", Some(ssl_json));
    cache.apply_change("plugin_configs", "pc1", Some(pc_json));

    let stats = cache.stats();
    assert_eq!(stats.routes, 1);
    assert_eq!(stats.services, 1);
    assert_eq!(stats.upstreams, 1);
    assert_eq!(stats.consumers, 1);
    assert_eq!(stats.ssl_certs, 1);
    assert_eq!(stats.plugin_configs, 1);
}
