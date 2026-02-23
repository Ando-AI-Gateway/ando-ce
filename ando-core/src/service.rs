use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Service definition — APISIX-compatible.
/// A service is a reusable bundle of upstream + plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    pub name: Option<String>,
    pub desc: Option<String>,

    /// Upstream reference.
    pub upstream_id: Option<String>,

    /// Inline upstream.
    pub upstream: Option<crate::upstream::Upstream>,

    /// Plugins applied to routes using this service.
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_service_deserializes() {
        let json = r#"{"id": "svc1"}"#;
        let svc: Service = serde_json::from_str(json).unwrap();
        assert_eq!(svc.id, "svc1");
        assert!(svc.name.is_none());
        assert!(svc.upstream_id.is_none());
        assert!(svc.plugins.is_empty());
    }

    #[test]
    fn full_service_roundtrip() {
        let svc = Service {
            id: "svc1".into(),
            name: Some("my-service".into()),
            desc: Some("Test service".into()),
            upstream_id: Some("ups1".into()),
            upstream: None,
            plugins: {
                let mut m = HashMap::new();
                m.insert("rate-limiting".into(), serde_json::json!({"count": 100}));
                m
            },
            labels: {
                let mut m = HashMap::new();
                m.insert("env".into(), "prod".into());
                m
            },
        };
        let json = serde_json::to_string(&svc).unwrap();
        let svc2: Service = serde_json::from_str(&json).unwrap();
        assert_eq!(svc2.id, "svc1");
        assert_eq!(svc2.name.as_deref(), Some("my-service"));
        assert_eq!(svc2.upstream_id.as_deref(), Some("ups1"));
        assert!(svc2.plugins.contains_key("rate-limiting"));
        assert_eq!(svc2.labels.get("env").unwrap(), "prod");
    }

    #[test]
    fn service_with_inline_upstream_roundtrip() {
        let json = serde_json::json!({
            "id": "svc2",
            "upstream": {
                "nodes": { "10.0.0.1:8080": 1 },
                "type": "roundrobin"
            }
        });
        let svc: Service = serde_json::from_value(json).unwrap();
        assert!(svc.upstream.is_some());
        let ups = svc.upstream.as_ref().unwrap();
        assert!(ups.nodes.contains_key("10.0.0.1:8080"));
    }
}
