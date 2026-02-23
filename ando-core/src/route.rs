use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Route definition — APISIX-compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: String,

    /// URI path pattern (e.g. "/api/v1/*" or "/exact/path").
    pub uri: String,

    /// HTTP methods (empty = all methods).
    #[serde(default)]
    pub methods: Vec<String>,

    /// Host matching (optional).
    #[serde(default)]
    pub hosts: Vec<String>,

    /// Inline upstream definition.
    pub upstream: Option<crate::upstream::Upstream>,

    /// Reference to a named upstream.
    pub upstream_id: Option<String>,

    /// Reference to a named service.
    pub service_id: Option<String>,

    /// Plugins applied to this route.
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    /// Reference to a reusable plugin config set.
    pub plugin_config_id: Option<String>,

    /// Route priority (higher = matched first for same path).
    #[serde(default)]
    pub priority: i32,

    /// Route status: 1 = enabled, 0 = disabled.
    #[serde(default = "default_status")]
    pub status: u8,

    /// Human-readable name.
    pub name: Option<String>,

    /// Description.
    pub desc: Option<String>,

    /// Labels for filtering.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

fn default_status() -> u8 { 1 }

impl Route {
    /// Returns true if this route has any plugins (from route-level config).
    pub fn has_plugins(&self) -> bool {
        !self.plugins.is_empty() || self.plugin_config_id.is_some() || self.service_id.is_some()
    }

    /// Check if a given HTTP method is allowed.
    pub fn matches_method(&self, method: &str) -> bool {
        self.methods.is_empty() || self.methods.iter().any(|m| m.eq_ignore_ascii_case(method))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_route(uri: &str, methods: Vec<&str>) -> Route {
        Route {
            id: "test".into(),
            uri: uri.into(),
            methods: methods.into_iter().map(|s| s.to_string()).collect(),
            hosts: vec![],
            upstream: None,
            upstream_id: None,
            service_id: None,
            plugins: Default::default(),
            plugin_config_id: None,
            priority: 0,
            status: 1,
            name: None,
            desc: None,
            labels: Default::default(),
        }
    }

    #[test]
    fn test_matches_method_empty_allows_all() {
        let route = make_route("/api", vec![]);
        assert!(route.matches_method("GET"));
        assert!(route.matches_method("POST"));
        assert!(route.matches_method("DELETE"));
        assert!(route.matches_method("PATCH"));
    }

    #[test]
    fn test_matches_method_specific() {
        let route = make_route("/api", vec!["GET", "POST"]);
        assert!(route.matches_method("GET"));
        assert!(route.matches_method("POST"));
        assert!(!route.matches_method("DELETE"));
        assert!(!route.matches_method("PUT"));
    }

    #[test]
    fn test_matches_method_case_insensitive() {
        let route = make_route("/api", vec!["GET"]);
        assert!(route.matches_method("get"));
        assert!(route.matches_method("Get"));
        assert!(route.matches_method("GET"));
    }

    #[test]
    fn test_has_plugins_empty() {
        let route = make_route("/api", vec![]);
        assert!(!route.has_plugins());
    }

    #[test]
    fn test_has_plugins_with_plugin_map() {
        let mut route = make_route("/api", vec![]);
        route.plugins.insert("key-auth".to_string(), serde_json::json!({}));
        assert!(route.has_plugins());
    }

    #[test]
    fn test_has_plugins_with_plugin_config_id() {
        let mut route = make_route("/api", vec![]);
        route.plugin_config_id = Some("cfg1".to_string());
        assert!(route.has_plugins());
    }

    #[test]
    fn test_has_plugins_with_service_id() {
        let mut route = make_route("/api", vec![]);
        route.service_id = Some("svc1".to_string());
        assert!(route.has_plugins());
    }

    #[test]
    fn test_default_status_enabled() {
        let json = r#"{"id":"r1","uri":"/test"}"#;
        let route: Route = serde_json::from_str(json).unwrap();
        assert_eq!(route.status, 1);
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut route = make_route("/api/users", vec!["GET", "POST"]);
        route.plugins.insert("key-auth".to_string(), serde_json::json!({"header": "x-api-key"}));
        route.upstream_id = Some("us1".to_string());
        let json = serde_json::to_string(&route).unwrap();
        let decoded: Route = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, route.id);
        assert_eq!(decoded.uri, route.uri);
        assert_eq!(decoded.methods, route.methods);
        assert_eq!(decoded.upstream_id, route.upstream_id);
        assert_eq!(decoded.plugins.len(), 1);
    }

    #[test]
    fn test_status_zero_disabled() {
        let json = r#"{"id":"r1","uri":"/test","status":0}"#;
        let route: Route = serde_json::from_str(json).unwrap();
        assert_eq!(route.status, 0);
    }
}
