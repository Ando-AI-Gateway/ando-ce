use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Reusable plugin config set — APISIX-compatible.
/// Can be referenced by multiple routes to share plugin configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub id: String,
    pub desc: Option<String>,

    /// Plugin configurations.
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_plugin_config_deserializes() {
        let json = r#"{"id":"pc1"}"#;
        let pc: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(pc.id, "pc1");
        assert!(pc.plugins.is_empty());
        assert!(pc.labels.is_empty());
    }

    #[test]
    fn full_plugin_config_roundtrip() {
        let pc = PluginConfig {
            id: "pc1".into(),
            desc: Some("shared auth".into()),
            plugins: {
                let mut m = HashMap::new();
                m.insert("key-auth".into(), serde_json::json!({}));
                m.insert("rate-limiting".into(), serde_json::json!({"count": 50, "time_window": 60}));
                m
            },
            labels: HashMap::new(),
        };
        let json = serde_json::to_string(&pc).unwrap();
        let pc2: PluginConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(pc2.id, "pc1");
        assert_eq!(pc2.desc.as_deref(), Some("shared auth"));
        assert_eq!(pc2.plugins.len(), 2);
    }
}
