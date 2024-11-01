use ando_store::schema::KeySchema;

// =============================================================================
// Key Schema Construction
// =============================================================================

#[test]
fn test_key_schema_new() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.routes_prefix(), "/ando/routes/");
}

#[test]
fn test_key_schema_trailing_slash_stripped() {
    let schema = KeySchema::new("/ando/");
    assert_eq!(schema.routes_prefix(), "/ando/routes/");
}

#[test]
fn test_key_schema_custom_prefix() {
    let schema = KeySchema::new("/custom/prefix");
    assert_eq!(schema.routes_prefix(), "/custom/prefix/routes/");
    assert_eq!(schema.route_key("r1"), "/custom/prefix/routes/r1");
}

// =============================================================================
// Key Generation Tests
// =============================================================================

#[test]
fn test_route_key() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.route_key("r1"), "/ando/routes/r1");
    assert_eq!(schema.route_key("my-route-123"), "/ando/routes/my-route-123");
}

#[test]
fn test_service_key() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.service_key("s1"), "/ando/services/s1");
}

#[test]
fn test_upstream_key() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.upstream_key("u1"), "/ando/upstreams/u1");
}

#[test]
fn test_consumer_key() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.consumer_key("c1"), "/ando/consumers/c1");
}

#[test]
fn test_ssl_key() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.ssl_key("ssl1"), "/ando/ssl/ssl1");
}

#[test]
fn test_plugin_config_key() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.plugin_config_key("pc1"), "/ando/plugin_configs/pc1");
}

// =============================================================================
// Prefix Tests
// =============================================================================

#[test]
fn test_routes_prefix() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.routes_prefix(), "/ando/routes/");
}

#[test]
fn test_services_prefix() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.services_prefix(), "/ando/services/");
}

#[test]
fn test_upstreams_prefix() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.upstreams_prefix(), "/ando/upstreams/");
}

#[test]
fn test_consumers_prefix() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.consumers_prefix(), "/ando/consumers/");
}

#[test]
fn test_ssl_prefix() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.ssl_prefix(), "/ando/ssl/");
}

#[test]
fn test_plugin_configs_prefix() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.plugin_configs_prefix(), "/ando/plugin_configs/");
}

#[test]
fn test_global_rules_prefix() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.global_rules_prefix(), "/ando/global_rules/");
}

#[test]
fn test_all_prefix() {
    let schema = KeySchema::new("/ando");
    assert_eq!(schema.all_prefix(), "/ando/");
}

// =============================================================================
// Parse Key Tests
// =============================================================================

#[test]
fn test_parse_key_routes() {
    let schema = KeySchema::new("/ando");
    let result = schema.parse_key("/ando/routes/r1");
    assert!(result.is_some());
    let (resource_type, id) = result.unwrap();
    assert_eq!(resource_type, "routes");
    assert_eq!(id, "r1");
}

#[test]
fn test_parse_key_services() {
    let schema = KeySchema::new("/ando");
    let (rtype, id) = schema.parse_key("/ando/services/svc-1").unwrap();
    assert_eq!(rtype, "services");
    assert_eq!(id, "svc-1");
}

#[test]
fn test_parse_key_upstreams() {
    let schema = KeySchema::new("/ando");
    let (rtype, id) = schema.parse_key("/ando/upstreams/u-1").unwrap();
    assert_eq!(rtype, "upstreams");
    assert_eq!(id, "u-1");
}

#[test]
fn test_parse_key_consumers() {
    let schema = KeySchema::new("/ando");
    let (rtype, id) = schema.parse_key("/ando/consumers/user1").unwrap();
    assert_eq!(rtype, "consumers");
    assert_eq!(id, "user1");
}

#[test]
fn test_parse_key_ssl() {
    let schema = KeySchema::new("/ando");
    let (rtype, id) = schema.parse_key("/ando/ssl/cert-1").unwrap();
    assert_eq!(rtype, "ssl");
    assert_eq!(id, "cert-1");
}

#[test]
fn test_parse_key_plugin_configs() {
    let schema = KeySchema::new("/ando");
    let (rtype, id) = schema.parse_key("/ando/plugin_configs/pc-1").unwrap();
    assert_eq!(rtype, "plugin_configs");
    assert_eq!(id, "pc-1");
}

#[test]
fn test_parse_key_global_rules() {
    let schema = KeySchema::new("/ando");
    let (rtype, id) = schema.parse_key("/ando/global_rules/gr-1").unwrap();
    assert_eq!(rtype, "global_rules");
    assert_eq!(id, "gr-1");
}

#[test]
fn test_parse_key_invalid_prefix() {
    let schema = KeySchema::new("/ando");
    let result = schema.parse_key("/other/routes/r1");
    assert!(result.is_none());
}

#[test]
fn test_parse_key_no_id() {
    let schema = KeySchema::new("/ando");
    // A key with just the resource type and no ID should return None
    let result = schema.parse_key("/ando/routes");
    assert!(result.is_none());
}

#[test]
fn test_parse_key_custom_prefix() {
    let schema = KeySchema::new("/myapp/gateway");
    let (rtype, id) = schema.parse_key("/myapp/gateway/routes/r1").unwrap();
    assert_eq!(rtype, "routes");
    assert_eq!(id, "r1");
}

// =============================================================================
// Round-trip Tests
// =============================================================================

#[test]
fn test_key_roundtrip_routes() {
    let schema = KeySchema::new("/ando");
    let key = schema.route_key("my-route");
    let (rtype, id) = schema.parse_key(&key).unwrap();
    assert_eq!(rtype, "routes");
    assert_eq!(id, "my-route");
}

#[test]
fn test_key_roundtrip_services() {
    let schema = KeySchema::new("/ando");
    let key = schema.service_key("my-svc");
    let (rtype, id) = schema.parse_key(&key).unwrap();
    assert_eq!(rtype, "services");
    assert_eq!(id, "my-svc");
}

#[test]
fn test_key_roundtrip_upstreams() {
    let schema = KeySchema::new("/ando");
    let key = schema.upstream_key("my-upstream");
    let (rtype, id) = schema.parse_key(&key).unwrap();
    assert_eq!(rtype, "upstreams");
    assert_eq!(id, "my-upstream");
}

#[test]
fn test_key_roundtrip_consumers() {
    let schema = KeySchema::new("/ando");
    let key = schema.consumer_key("user123");
    let (rtype, id) = schema.parse_key(&key).unwrap();
    assert_eq!(rtype, "consumers");
    assert_eq!(id, "user123");
}

#[test]
fn test_key_roundtrip_ssl() {
    let schema = KeySchema::new("/ando");
    let key = schema.ssl_key("cert-abc");
    let (rtype, id) = schema.parse_key(&key).unwrap();
    assert_eq!(rtype, "ssl");
    assert_eq!(id, "cert-abc");
}

#[test]
fn test_key_roundtrip_plugin_configs() {
    let schema = KeySchema::new("/ando");
    let key = schema.plugin_config_key("pc-xyz");
    let (rtype, id) = schema.parse_key(&key).unwrap();
    assert_eq!(rtype, "plugin_configs");
    assert_eq!(id, "pc-xyz");
}
