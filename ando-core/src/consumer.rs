use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Consumer definition — APISIX-compatible.
/// Represents an API consumer with authentication credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consumer {
    pub username: String,

    /// Plugins with consumer-specific config (e.g. key-auth key).
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    pub desc: Option<String>,

    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consumer_minimal() {
        let json = r#"{"username":"bob"}"#;
        let c: Consumer = serde_json::from_str(json).unwrap();
        assert_eq!(c.username, "bob");
        assert!(c.plugins.is_empty());
        assert!(c.desc.is_none());
        assert!(c.labels.is_empty());
    }

    #[test]
    fn test_consumer_with_key_auth_plugin() {
        let json = r#"{"username":"alice","plugins":{"key-auth":{"key":"secret-key"}}}"#;
        let c: Consumer = serde_json::from_str(json).unwrap();
        assert_eq!(c.username, "alice");
        let key_auth = c.plugins.get("key-auth").expect("key-auth plugin must be present");
        assert_eq!(key_auth["key"], "secret-key");
    }

    #[test]
    fn test_consumer_serde_roundtrip() {
        let mut c = Consumer {
            username: "alice".into(),
            plugins: Default::default(),
            desc: Some("test user".into()),
            labels: Default::default(),
        };
        c.plugins.insert("key-auth".into(), serde_json::json!({"key": "s3cr3t"}));
        c.labels.insert("env".into(), "prod".into());
        let json = serde_json::to_string(&c).unwrap();
        let decoded: Consumer = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.username, "alice");
        assert_eq!(decoded.desc, Some("test user".into()));
        assert_eq!(decoded.labels.get("env").map(|s| s.as_str()), Some("prod"));
        assert_eq!(decoded.plugins["key-auth"]["key"], "s3cr3t");
    }

    #[test]
    fn test_consumer_multiple_plugins() {
        let json = r#"{"username":"carol","plugins":{"key-auth":{"key":"k1"},"rate-limiting":{"count":100,"time_window":60}}}"#;
        let c: Consumer = serde_json::from_str(json).unwrap();
        assert_eq!(c.plugins.len(), 2);
        assert!(c.plugins.contains_key("key-auth"));
        assert!(c.plugins.contains_key("rate-limiting"));
    }
}
