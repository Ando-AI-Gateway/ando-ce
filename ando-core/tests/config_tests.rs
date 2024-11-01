use ando_core::config::*;
use std::net::SocketAddr;

// =============================================================================
// Default Config Tests
// =============================================================================

#[test]
fn test_default_config_all_fields() {
    let cfg = AndoConfig::default();

    // Node ID should be a non-empty UUID
    assert!(!cfg.node_id.is_empty());

    // Proxy defaults
    assert_eq!(cfg.proxy.http_addr, "0.0.0.0:9080".parse::<SocketAddr>().unwrap());
    assert_eq!(cfg.proxy.https_addr, "0.0.0.0:9443".parse::<SocketAddr>().unwrap());
    assert_eq!(cfg.proxy.workers, 0);
    assert!(cfg.proxy.http2);
    assert!(cfg.proxy.grpc);
    assert!(cfg.proxy.websocket);
    assert_eq!(cfg.proxy.body_buffer_size, 64 * 1024);
    assert_eq!(cfg.proxy.connect_timeout_ms, 6000);
    assert_eq!(cfg.proxy.read_timeout_ms, 15000);
    assert_eq!(cfg.proxy.write_timeout_ms, 15000);

    // Admin defaults
    assert_eq!(cfg.admin.addr, "127.0.0.1:9180".parse::<SocketAddr>().unwrap());
    assert!(cfg.admin.api_key.is_none());
    assert!(cfg.admin.enabled);
    assert!(cfg.admin.cors_origins.is_empty());

    // etcd defaults
    assert_eq!(cfg.etcd.endpoints, vec!["http://127.0.0.1:2379".to_string()]);
    assert_eq!(cfg.etcd.prefix, "/ando");
    assert_eq!(cfg.etcd.timeout_ms, 5000);
    assert!(cfg.etcd.username.is_none());
    assert!(cfg.etcd.password.is_none());
    assert!(cfg.etcd.tls.is_none());

    // Observability defaults
    assert!(!cfg.observability.victoria_metrics.enabled);
    assert_eq!(
        cfg.observability.victoria_metrics.endpoint,
        "http://127.0.0.1:8428/api/v1/write"
    );
    assert_eq!(cfg.observability.victoria_metrics.push_interval_secs, 15);
    assert!(cfg.observability.victoria_metrics.labels.is_empty());

    assert!(!cfg.observability.victoria_logs.enabled);
    assert_eq!(
        cfg.observability.victoria_logs.endpoint,
        "http://127.0.0.1:9428/insert/jsonline"
    );
    assert_eq!(cfg.observability.victoria_logs.batch_size, 1000);
    assert_eq!(cfg.observability.victoria_logs.flush_interval_secs, 5);

    assert!(cfg.observability.prometheus.enabled);
    assert_eq!(cfg.observability.prometheus.path, "/metrics");

    // Lua defaults
    assert_eq!(cfg.lua.plugin_dir.to_str().unwrap(), "/etc/ando/plugins");
    assert_eq!(cfg.lua.pool_size, 32);
    assert_eq!(cfg.lua.timeout_ms, 5000);
    assert_eq!(cfg.lua.max_memory, 64 * 1024 * 1024);
    assert!(cfg.lua.package_path.is_empty());

    // Deployment defaults
    assert_eq!(cfg.deployment.mode, DeploymentMode::Standard);
    assert!(cfg.deployment.config_file.is_none());
}

#[test]
fn test_default_proxy_config() {
    let proxy = ProxyConfig::default();
    assert_eq!(proxy.http_addr.port(), 9080);
    assert_eq!(proxy.https_addr.port(), 9443);
    assert!(proxy.http2);
    assert!(proxy.grpc);
    assert!(proxy.websocket);
}

#[test]
fn test_default_admin_config() {
    let admin = AdminConfig::default();
    assert_eq!(admin.addr.port(), 9180);
    assert!(admin.enabled);
    assert!(admin.api_key.is_none());
}

#[test]
fn test_default_etcd_config() {
    let etcd = EtcdConfig::default();
    assert_eq!(etcd.endpoints.len(), 1);
    assert_eq!(etcd.prefix, "/ando");
    assert_eq!(etcd.timeout_ms, 5000);
}

#[test]
fn test_default_lua_config() {
    let lua = LuaConfig::default();
    assert_eq!(lua.pool_size, 32);
    assert_eq!(lua.timeout_ms, 5000);
    assert_eq!(lua.max_memory, 64 * 1024 * 1024);
}

// =============================================================================
// Deployment Mode Tests
// =============================================================================

#[test]
fn test_deployment_mode_standard() {
    let cfg = AndoConfig::default();
    assert!(!cfg.is_standalone());
    assert!(!cfg.is_edge());
}

#[test]
fn test_deployment_mode_standalone() {
    let mut cfg = AndoConfig::default();
    cfg.deployment.mode = DeploymentMode::Standalone;
    assert!(cfg.is_standalone());
    assert!(!cfg.is_edge());
}

#[test]
fn test_deployment_mode_edge() {
    let mut cfg = AndoConfig::default();
    cfg.deployment.mode = DeploymentMode::Edge;
    assert!(cfg.is_standalone()); // edge is also standalone
    assert!(cfg.is_edge());
}

#[test]
fn test_deployment_mode_default() {
    let mode = DeploymentMode::default();
    assert_eq!(mode, DeploymentMode::Standard);
}

// =============================================================================
// YAML Deserialization Tests
// =============================================================================

#[test]
fn test_deserialize_minimal_yaml() {
    let yaml = r#"
proxy:
  http_addr: "0.0.0.0:8080"
"#;
    let cfg: AndoConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.proxy.http_addr.port(), 8080);
    // Other fields should have defaults
    assert_eq!(cfg.proxy.https_addr.port(), 9443);
    assert!(cfg.admin.enabled);
}

#[test]
fn test_deserialize_full_yaml() {
    let yaml = r#"
node_id: "test-node"
proxy:
  http_addr: "0.0.0.0:8080"
  https_addr: "0.0.0.0:8443"
  workers: 4
  http2: false
  grpc: false
  websocket: false
  body_buffer_size: 131072
  connect_timeout_ms: 3000
  read_timeout_ms: 5000
  write_timeout_ms: 5000
admin:
  addr: "127.0.0.1:8180"
  api_key: "test-key"
  enabled: false
etcd:
  endpoints:
    - "http://etcd1:2379"
    - "http://etcd2:2379"
  prefix: "/test"
  timeout_ms: 3000
  username: "root"
  password: "secret"
deployment:
  mode: edge
"#;
    let cfg: AndoConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.node_id, "test-node");
    assert_eq!(cfg.proxy.http_addr.port(), 8080);
    assert_eq!(cfg.proxy.https_addr.port(), 8443);
    assert_eq!(cfg.proxy.workers, 4);
    assert!(!cfg.proxy.http2);
    assert!(!cfg.proxy.grpc);
    assert!(!cfg.proxy.websocket);
    assert_eq!(cfg.proxy.body_buffer_size, 131072);
    assert_eq!(cfg.proxy.connect_timeout_ms, 3000);
    assert_eq!(cfg.proxy.read_timeout_ms, 5000);
    assert_eq!(cfg.proxy.write_timeout_ms, 5000);
    assert_eq!(cfg.admin.addr.port(), 8180);
    assert_eq!(cfg.admin.api_key.as_deref(), Some("test-key"));
    assert!(!cfg.admin.enabled);
    assert_eq!(cfg.etcd.endpoints.len(), 2);
    assert_eq!(cfg.etcd.prefix, "/test");
    assert_eq!(cfg.etcd.timeout_ms, 3000);
    assert_eq!(cfg.etcd.username.as_deref(), Some("root"));
    assert_eq!(cfg.etcd.password.as_deref(), Some("secret"));
    assert_eq!(cfg.deployment.mode, DeploymentMode::Edge);
    assert!(cfg.is_edge());
}

#[test]
fn test_deserialize_deployment_modes() {
    let yaml_standard = r#"
deployment:
  mode: standard
"#;
    let cfg: AndoConfig = serde_yaml::from_str(yaml_standard).unwrap();
    assert_eq!(cfg.deployment.mode, DeploymentMode::Standard);

    let yaml_standalone = r#"
deployment:
  mode: standalone
"#;
    let cfg: AndoConfig = serde_yaml::from_str(yaml_standalone).unwrap();
    assert_eq!(cfg.deployment.mode, DeploymentMode::Standalone);

    let yaml_edge = r#"
deployment:
  mode: edge
"#;
    let cfg: AndoConfig = serde_yaml::from_str(yaml_edge).unwrap();
    assert_eq!(cfg.deployment.mode, DeploymentMode::Edge);
}

#[test]
fn test_deserialize_observability_config() {
    let yaml = r#"
observability:
  victoria_metrics:
    enabled: true
    endpoint: "http://vm:8428/api/v1/write"
    push_interval_secs: 30
    labels:
      env: "staging"
      cluster: "us-west"
  victoria_logs:
    enabled: true
    endpoint: "http://vl:9428/insert/jsonline"
    batch_size: 500
    flush_interval_secs: 10
  prometheus:
    enabled: false
    path: "/prom"
"#;
    let cfg: AndoConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(cfg.observability.victoria_metrics.enabled);
    assert_eq!(cfg.observability.victoria_metrics.endpoint, "http://vm:8428/api/v1/write");
    assert_eq!(cfg.observability.victoria_metrics.push_interval_secs, 30);
    assert_eq!(cfg.observability.victoria_metrics.labels.get("env").unwrap(), "staging");
    assert_eq!(cfg.observability.victoria_metrics.labels.get("cluster").unwrap(), "us-west");
    assert!(cfg.observability.victoria_logs.enabled);
    assert_eq!(cfg.observability.victoria_logs.batch_size, 500);
    assert_eq!(cfg.observability.victoria_logs.flush_interval_secs, 10);
    assert!(!cfg.observability.prometheus.enabled);
    assert_eq!(cfg.observability.prometheus.path, "/prom");
}

// =============================================================================
// Serialization Roundtrip Tests
// =============================================================================

#[test]
fn test_config_serialization_roundtrip() {
    let original = AndoConfig::default();
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: AndoConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(original.proxy.http_addr, deserialized.proxy.http_addr);
    assert_eq!(original.proxy.https_addr, deserialized.proxy.https_addr);
    assert_eq!(original.admin.addr, deserialized.admin.addr);
    assert_eq!(original.etcd.prefix, deserialized.etcd.prefix);
    assert_eq!(original.deployment.mode, deserialized.deployment.mode);
}

// =============================================================================
// Load Config Tests
// =============================================================================

#[test]
fn test_load_nonexistent_config_file_uses_defaults() {
    // Loading with a nonexistent path won't fail - figment is permissive,
    // it just won't merge any file. So we get defaults.
    let cfg = AndoConfig::load(Some("/tmp/nonexistent_ando_test_config.yaml"));
    assert!(cfg.is_ok());
    let cfg = cfg.unwrap();
    assert_eq!(cfg.proxy.http_addr.port(), 9080);
}

#[test]
fn test_load_from_yaml_file() {
    // Create a temp file
    let yaml = r#"
node_id: "yaml-test-node"
proxy:
  http_addr: "0.0.0.0:7777"
"#;
    let path = "/tmp/ando_test_config.yaml";
    std::fs::write(path, yaml).unwrap();

    let cfg = AndoConfig::load(Some(path)).unwrap();
    assert_eq!(cfg.node_id, "yaml-test-node");
    assert_eq!(cfg.proxy.http_addr.port(), 7777);

    std::fs::remove_file(path).ok();
}

// =============================================================================
// Unique Node ID Tests
// =============================================================================

#[test]
fn test_default_node_id_is_unique() {
    let cfg1 = AndoConfig::default();
    let cfg2 = AndoConfig::default();
    assert_ne!(cfg1.node_id, cfg2.node_id, "Each default config should get a unique node_id");
}
