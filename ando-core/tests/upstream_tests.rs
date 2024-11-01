use ando_core::upstream::*;
use std::collections::HashMap;

// =============================================================================
// LoadBalancerType Tests
// =============================================================================

#[test]
fn test_load_balancer_type_default() {
    let lb = LoadBalancerType::default();
    assert_eq!(lb, LoadBalancerType::Roundrobin);
}

#[test]
fn test_load_balancer_type_serialization() {
    assert_eq!(serde_json::to_string(&LoadBalancerType::Roundrobin).unwrap(), "\"roundrobin\"");
    assert_eq!(serde_json::to_string(&LoadBalancerType::Chash).unwrap(), "\"chash\"");
    assert_eq!(serde_json::to_string(&LoadBalancerType::Ewma).unwrap(), "\"ewma\"");
    assert_eq!(serde_json::to_string(&LoadBalancerType::LeastConn).unwrap(), "\"least_conn\"");
}

#[test]
fn test_load_balancer_type_deserialization() {
    assert_eq!(
        serde_json::from_str::<LoadBalancerType>("\"roundrobin\"").unwrap(),
        LoadBalancerType::Roundrobin
    );
    assert_eq!(
        serde_json::from_str::<LoadBalancerType>("\"chash\"").unwrap(),
        LoadBalancerType::Chash
    );
    assert_eq!(
        serde_json::from_str::<LoadBalancerType>("\"ewma\"").unwrap(),
        LoadBalancerType::Ewma
    );
    assert_eq!(
        serde_json::from_str::<LoadBalancerType>("\"least_conn\"").unwrap(),
        LoadBalancerType::LeastConn
    );
}

// =============================================================================
// PassHostMode Tests
// =============================================================================

#[test]
fn test_pass_host_mode_default() {
    let mode = PassHostMode::default();
    assert_eq!(mode, PassHostMode::Pass);
}

#[test]
fn test_pass_host_mode_serialization() {
    assert_eq!(serde_json::to_string(&PassHostMode::Pass).unwrap(), "\"pass\"");
    assert_eq!(serde_json::to_string(&PassHostMode::Node).unwrap(), "\"node\"");
    assert_eq!(serde_json::to_string(&PassHostMode::Rewrite).unwrap(), "\"rewrite\"");
}

// =============================================================================
// Upstream Serialization Tests
// =============================================================================

#[test]
fn test_upstream_minimal_deserialization() {
    let json = r#"{"id":"u1","nodes":{"127.0.0.1:8080":1}}"#;
    let upstream: Upstream = serde_json::from_str(json).unwrap();
    assert_eq!(upstream.id, "u1");
    assert_eq!(upstream.r#type, LoadBalancerType::Roundrobin);
    assert_eq!(upstream.retries, 1);
    assert_eq!(upstream.scheme, "http");
    assert_eq!(upstream.pass_host, PassHostMode::Pass);
    assert!(upstream.checks.is_none());
    assert!(upstream.discovery.is_none());
    assert!(upstream.tls.is_none());
}

#[test]
fn test_upstream_full_deserialization() {
    let json = r#"{
        "id": "u1",
        "name": "backend-pool",
        "description": "Production backend",
        "type": "chash",
        "hash_on": "header",
        "key": "X-Session-ID",
        "nodes": {
            "10.0.0.1:80": 5,
            "10.0.0.2:80": 3,
            "10.0.0.3:80": 2
        },
        "retries": 3,
        "retry_timeout": 2.5,
        "scheme": "https",
        "pass_host": "rewrite",
        "upstream_host": "backend.internal",
        "labels": {
            "env": "prod",
            "team": "platform"
        }
    }"#;
    let upstream: Upstream = serde_json::from_str(json).unwrap();
    assert_eq!(upstream.id, "u1");
    assert_eq!(upstream.name, "backend-pool");
    assert_eq!(upstream.description, "Production backend");
    assert_eq!(upstream.r#type, LoadBalancerType::Chash);
    assert_eq!(upstream.hash_on.as_deref(), Some("header"));
    assert_eq!(upstream.key.as_deref(), Some("X-Session-ID"));
    assert_eq!(upstream.nodes.len(), 3);
    assert_eq!(*upstream.nodes.get("10.0.0.1:80").unwrap(), 5);
    assert_eq!(upstream.retries, 3);
    assert_eq!(upstream.retry_timeout, Some(2.5));
    assert_eq!(upstream.scheme, "https");
    assert_eq!(upstream.pass_host, PassHostMode::Rewrite);
    assert_eq!(upstream.upstream_host.as_deref(), Some("backend.internal"));
    assert_eq!(upstream.labels.get("env").unwrap(), "prod");
}

#[test]
fn test_upstream_roundtrip() {
    let upstream = Upstream {
        id: "u1".to_string(),
        name: "test".to_string(),
        description: String::new(),
        r#type: LoadBalancerType::LeastConn,
        hash_on: None,
        key: None,
        nodes: HashMap::from([("node1:80".to_string(), 10), ("node2:80".to_string(), 5)]),
        retries: 2,
        retry_timeout: Some(5.0),
        timeout: None,
        scheme: "http".to_string(),
        pass_host: PassHostMode::Node,
        upstream_host: None,
        checks: None,
        discovery: None,
        tls: None,
        labels: HashMap::new(),
        created_at: None,
        updated_at: None,
    };

    let json = serde_json::to_string(&upstream).unwrap();
    let deserialized: Upstream = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "u1");
    assert_eq!(deserialized.r#type, LoadBalancerType::LeastConn);
    assert_eq!(deserialized.pass_host, PassHostMode::Node);
    assert_eq!(deserialized.retries, 2);
}

// =============================================================================
// HealthCheckConfig Tests
// =============================================================================

#[test]
fn test_health_check_config_deserialization() {
    let json = r#"{
        "active": {
            "type": "http",
            "interval": 10,
            "timeout": 2.0,
            "http_path": "/healthz",
            "healthy_statuses": [200],
            "healthy_successes": 3,
            "unhealthy_failures": 5
        },
        "passive": {
            "healthy": {
                "successes": 3
            },
            "unhealthy": {
                "failures": 3,
                "tcp_failures": 1,
                "timeouts": 3
            }
        }
    }"#;
    let config: HealthCheckConfig = serde_json::from_str(json).unwrap();

    let active = config.active.unwrap();
    assert_eq!(active.r#type, "http");
    assert_eq!(active.interval, 10);
    assert_eq!(active.timeout, 2.0);
    assert_eq!(active.http_path, "/healthz");
    assert_eq!(active.healthy_statuses, vec![200]);
    assert_eq!(active.healthy_successes, 3);
    assert_eq!(active.unhealthy_failures, 5);

    let passive = config.passive.unwrap();
    assert_eq!(passive.healthy.successes, 3);
    assert_eq!(passive.unhealthy.failures, 3);
    assert_eq!(passive.unhealthy.tcp_failures, 1);
    assert_eq!(passive.unhealthy.timeouts, 3);
}

#[test]
fn test_active_health_check_defaults() {
    let json = "{}";
    let check: ActiveHealthCheck = serde_json::from_str(json).unwrap();
    assert_eq!(check.r#type, "http");
    assert_eq!(check.interval, 5);
    assert_eq!(check.timeout, 1.0);
    assert_eq!(check.http_path, "/");
    assert_eq!(check.healthy_statuses, vec![200, 302]);
    assert_eq!(check.healthy_successes, 2);
    assert_eq!(check.unhealthy_failures, 3);
}

#[test]
fn test_passive_healthy_serde_defaults() {
    // serde defaults are applied during deserialization, not via Default::default()
    let config: PassiveHealthyConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(config.successes, 5);
    // 200..=399
    assert_eq!(config.http_statuses.len(), 200);
    assert!(config.http_statuses.contains(&200));
    assert!(config.http_statuses.contains(&399));
}

#[test]
fn test_passive_unhealthy_serde_defaults() {
    // serde defaults are applied during deserialization, not via Default::default()
    let config: PassiveUnhealthyConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(config.failures, 5);
    assert_eq!(config.tcp_failures, 2);
    assert_eq!(config.timeouts, 7);
    assert_eq!(config.http_statuses, vec![500, 502, 503, 504]);
}

// =============================================================================
// DiscoveryConfig Tests
// =============================================================================

#[test]
fn test_discovery_config_deserialization() {
    let json = r#"{
        "type": "consul",
        "service_name": "my-backend",
        "args": {
            "datacenter": "dc1",
            "tag": "primary"
        }
    }"#;
    let config: DiscoveryConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.r#type, "consul");
    assert_eq!(config.service_name, "my-backend");
    assert_eq!(config.args.get("datacenter").unwrap(), "dc1");
    assert_eq!(config.args.get("tag").unwrap(), "primary");
}

// =============================================================================
// UpstreamTls Tests
// =============================================================================

#[test]
fn test_upstream_tls_deserialization() {
    let json = r#"{
        "verify": true,
        "ca_cert": "/path/to/ca.pem",
        "client_cert": "/path/to/cert.pem",
        "client_key": "/path/to/key.pem"
    }"#;
    let tls: UpstreamTls = serde_json::from_str(json).unwrap();
    assert!(tls.verify);
    assert_eq!(tls.ca_cert.as_deref(), Some("/path/to/ca.pem"));
    assert_eq!(tls.client_cert.as_deref(), Some("/path/to/cert.pem"));
    assert_eq!(tls.client_key.as_deref(), Some("/path/to/key.pem"));
}

#[test]
fn test_upstream_tls_minimal() {
    let json = "{}";
    let tls: UpstreamTls = serde_json::from_str(json).unwrap();
    assert!(!tls.verify);
    assert!(tls.ca_cert.is_none());
    assert!(tls.client_cert.is_none());
    assert!(tls.client_key.is_none());
}
