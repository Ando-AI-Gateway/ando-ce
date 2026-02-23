use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Upstream target definition — APISIX-compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub id: Option<String>,

    /// Upstream name.
    pub name: Option<String>,

    /// Load balancer type: roundrobin, chash, ewma.
    #[serde(default = "default_lb_type", rename = "type")]
    pub lb_type: String,

    /// Nodes: address → weight.
    #[serde(default)]
    pub nodes: HashMap<String, u32>,

    /// Health check config.
    pub health_check: Option<HealthCheck>,

    /// Connection timeout override (ms).
    pub connect_timeout_ms: Option<u64>,

    /// Read/write timeouts (ms).
    pub read_timeout_ms: Option<u64>,
    pub write_timeout_ms: Option<u64>,

    /// Pass host mode: "pass" | "node" | "rewrite".
    #[serde(default = "default_pass_host")]
    pub pass_host: String,

    /// Upstream host header (used when pass_host = "rewrite").
    pub upstream_host: Option<String>,

    /// Retries on upstream failure.
    #[serde(default = "default_retries")]
    pub retries: u32,

    /// Description.
    pub desc: Option<String>,

    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    #[serde(default)]
    pub active: Option<ActiveHealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveHealthCheck {
    #[serde(default = "default_hc_type")]
    pub r#type: String,
    #[serde(default = "default_hc_interval")]
    pub interval: u64,
    #[serde(default = "default_hc_timeout")]
    pub timeout: u64,
    pub http_path: Option<String>,
    #[serde(default = "default_healthy_successes")]
    pub healthy_successes: u32,
    #[serde(default = "default_unhealthy_failures")]
    pub unhealthy_failures: u32,
}

fn default_lb_type() -> String { "roundrobin".into() }
fn default_pass_host() -> String { "pass".into() }
fn default_retries() -> u32 { 1 }
fn default_hc_type() -> String { "http".into() }
fn default_hc_interval() -> u64 { 5 }
fn default_hc_timeout() -> u64 { 3 }
fn default_healthy_successes() -> u32 { 2 }
fn default_unhealthy_failures() -> u32 { 3 }

impl Upstream {
    /// Get the first node address (for single-node upstreams).
    pub fn first_node(&self) -> Option<&str> {
        self.nodes.keys().next().map(|s| s.as_str())
    }

    /// Returns true if there are no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_upstream(nodes: Vec<(&str, u32)>) -> Upstream {
        Upstream {
            id: Some("us1".into()),
            name: Some("test".into()),
            lb_type: "roundrobin".into(),
            nodes: nodes.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            health_check: None,
            connect_timeout_ms: None,
            read_timeout_ms: None,
            write_timeout_ms: None,
            pass_host: "pass".into(),
            upstream_host: None,
            retries: 1,
            desc: None,
            labels: Default::default(),
        }
    }

    #[test]
    fn test_first_node_empty() {
        let us = make_upstream(vec![]);
        assert!(us.first_node().is_none());
        assert!(us.is_empty());
    }

    #[test]
    fn test_first_node_single() {
        let us = make_upstream(vec![("127.0.0.1:8080", 1)]);
        assert_eq!(us.first_node(), Some("127.0.0.1:8080"));
        assert!(!us.is_empty());
    }

    #[test]
    fn test_defaults_from_serde() {
        let json = r#"{"nodes":{"127.0.0.1:8080":1}}"#;
        let us: Upstream = serde_json::from_str(json).unwrap();
        assert_eq!(us.lb_type, "roundrobin");
        assert_eq!(us.pass_host, "pass");
        assert_eq!(us.retries, 1);
    }

    #[test]
    fn test_serde_roundtrip_multiple_nodes() {
        let us = make_upstream(vec![("10.0.0.1:9000", 100), ("10.0.0.2:9000", 50)]);
        let json = serde_json::to_string(&us).unwrap();
        let decoded: Upstream = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.nodes.len(), 2);
        assert_eq!(decoded.nodes.get("10.0.0.1:9000"), Some(&100));
        assert_eq!(decoded.nodes.get("10.0.0.2:9000"), Some(&50));
    }

    #[test]
    fn test_weighted_nodes() {
        let us = make_upstream(vec![("a:80", 10), ("b:80", 20), ("c:80", 30)]);
        assert_eq!(us.nodes.len(), 3);
        assert_eq!(us.nodes["a:80"], 10);
        assert_eq!(us.nodes["b:80"], 20);
        assert_eq!(us.nodes["c:80"], 30);
    }

    #[test]
    fn test_health_check_defaults() {
        let json = r#"{"nodes":{"127.0.0.1:8080":1},"health_check":{"active":{}}}"#;
        let us: Upstream = serde_json::from_str(json).unwrap();
        let hc = us.health_check.unwrap();
        let active = hc.active.unwrap();
        assert_eq!(active.r#type, "http");
        assert_eq!(active.interval, 5);
        assert_eq!(active.timeout, 3);
        assert_eq!(active.healthy_successes, 2);
        assert_eq!(active.unhealthy_failures, 3);
    }
}
