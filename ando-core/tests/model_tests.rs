use ando_core::consumer::Consumer;
use ando_core::plugin_config::PluginConfig;
use ando_core::service::Service;
use ando_core::ssl::SslCert;
use std::collections::HashMap;

// =============================================================================
// Consumer Tests
// =============================================================================

#[test]
fn test_consumer_serialization_roundtrip() {
    let consumer = Consumer {
        id: "c1".to_string(),
        username: "test-user".to_string(),
        description: "A test consumer".to_string(),
        plugins: HashMap::from([(
            "key-auth".to_string(),
            serde_json::json!({"key": "abc123"}),
        )]),
        group: Some("vip".to_string()),
        labels: HashMap::from([("tier".to_string(), "premium".to_string())]),
        created_at: None,
        updated_at: None,
    };

    let json = serde_json::to_string(&consumer).unwrap();
    let deserialized: Consumer = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "c1");
    assert_eq!(deserialized.username, "test-user");
    assert_eq!(deserialized.description, "A test consumer");
    assert!(deserialized.plugins.contains_key("key-auth"));
    assert_eq!(deserialized.group, Some("vip".to_string()));
    assert_eq!(deserialized.labels.get("tier").unwrap(), "premium");
}

#[test]
fn test_consumer_minimal_deserialization() {
    let json = r#"{"id":"c1","username":"user1"}"#;
    let consumer: Consumer = serde_json::from_str(json).unwrap();
    assert_eq!(consumer.id, "c1");
    assert_eq!(consumer.username, "user1");
    assert!(consumer.description.is_empty());
    assert!(consumer.plugins.is_empty());
    assert!(consumer.group.is_none());
    assert!(consumer.labels.is_empty());
}

// =============================================================================
// Service Tests
// =============================================================================

#[test]
fn test_service_serialization_roundtrip() {
    let service = Service {
        id: "s1".to_string(),
        name: "backend-service".to_string(),
        description: "Main backend".to_string(),
        upstream: None,
        upstream_id: Some("u1".to_string()),
        plugins: HashMap::from([(
            "cors".to_string(),
            serde_json::json!({"allow_origins": "*"}),
        )]),
        enable: true,
        labels: HashMap::new(),
        created_at: None,
        updated_at: None,
    };

    let json = serde_json::to_string(&service).unwrap();
    let deserialized: Service = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "s1");
    assert_eq!(deserialized.name, "backend-service");
    assert_eq!(deserialized.upstream_id, Some("u1".to_string()));
    assert!(deserialized.enable);
    assert!(deserialized.plugins.contains_key("cors"));
}

#[test]
fn test_service_minimal_deserialization() {
    let json = r#"{"id":"s1"}"#;
    let service: Service = serde_json::from_str(json).unwrap();
    assert_eq!(service.id, "s1");
    assert!(service.name.is_empty());
    assert!(service.upstream.is_none());
    assert!(service.upstream_id.is_none());
    assert!(service.enable); // default true
}

// =============================================================================
// SslCert Tests
// =============================================================================

#[test]
fn test_ssl_cert_serialization_roundtrip() {
    let cert = SslCert {
        id: "ssl1".to_string(),
        snis: vec!["example.com".to_string(), "*.example.com".to_string()],
        cert: "-----BEGIN CERTIFICATE-----\nMIICpD...\n-----END CERTIFICATE-----".to_string(),
        key: "-----BEGIN PRIVATE KEY-----\nMIIEvA...\n-----END PRIVATE KEY-----".to_string(),
        client_cert: None,
        status: true,
        validity_end: None,
        labels: HashMap::new(),
        created_at: None,
        updated_at: None,
    };

    let json = serde_json::to_string(&cert).unwrap();
    let deserialized: SslCert = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "ssl1");
    assert_eq!(deserialized.snis.len(), 2);
    assert!(deserialized.snis.contains(&"example.com".to_string()));
    assert!(deserialized.snis.contains(&"*.example.com".to_string()));
    assert!(!deserialized.cert.is_empty());
    assert!(!deserialized.key.is_empty());
    assert!(deserialized.status);
}

#[test]
fn test_ssl_cert_minimal() {
    let json = r#"{
        "id": "ssl1",
        "snis": ["test.com"],
        "cert": "cert-data",
        "key": "key-data"
    }"#;
    let cert: SslCert = serde_json::from_str(json).unwrap();
    assert_eq!(cert.id, "ssl1");
    assert!(cert.status); // default true
    assert!(cert.client_cert.is_none());
    assert!(cert.validity_end.is_none());
}

// =============================================================================
// PluginConfig Tests
// =============================================================================

#[test]
fn test_plugin_config_serialization_roundtrip() {
    let config = PluginConfig {
        id: "pc1".to_string(),
        description: "Shared auth config".to_string(),
        plugins: HashMap::from([
            ("key-auth".to_string(), serde_json::json!({"header": "X-API-KEY"})),
            ("cors".to_string(), serde_json::json!({"allow_origins": "*"})),
        ]),
        labels: HashMap::from([("env".to_string(), "production".to_string())]),
        created_at: None,
        updated_at: None,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: PluginConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "pc1");
    assert_eq!(deserialized.description, "Shared auth config");
    assert_eq!(deserialized.plugins.len(), 2);
    assert!(deserialized.plugins.contains_key("key-auth"));
    assert!(deserialized.plugins.contains_key("cors"));
    assert_eq!(deserialized.labels.get("env").unwrap(), "production");
}

#[test]
fn test_plugin_config_minimal() {
    let json = r#"{"id":"pc1","plugins":{"limit-count":{"count":100}}}"#;
    let config: PluginConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.id, "pc1");
    assert_eq!(config.plugins.len(), 1);
    assert!(config.description.is_empty());
    assert!(config.labels.is_empty());
}
