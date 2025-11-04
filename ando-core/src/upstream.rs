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
}
