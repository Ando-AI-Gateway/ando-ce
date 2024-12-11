use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An Upstream defines backend server pools and load-balancing strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    /// Unique identifier
    pub id: String,

    /// Human-readable name
    #[serde(default)]
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Load balancer type: "roundrobin", "chash", "ewma", "least_conn"
    #[serde(default = "default_lb_type")]
    pub r#type: LoadBalancerType,

    /// Consistent hash key (when type = chash)
    #[serde(default)]
    pub hash_on: Option<String>,

    /// Key for consistent hash
    #[serde(default)]
    pub key: Option<String>,

    /// Backend nodes: "host:port" -> weight
    #[serde(default)]
    pub nodes: HashMap<String, u32>,

    /// Number of retries on failure
    #[serde(default = "default_retries")]
    pub retries: u32,

    /// Retry timeout (seconds)
    #[serde(default)]
    pub retry_timeout: Option<f64>,

    /// Timeout configuration
    #[serde(default)]
    pub timeout: Option<crate::route::TimeoutConfig>,

    /// Scheme: "http", "https", "grpc", "grpcs"
    #[serde(default = "default_scheme")]
    pub scheme: String,

    /// Pass host mode: "pass", "node", "rewrite"
    #[serde(default = "default_pass_host")]
    pub pass_host: PassHostMode,

    /// Upstream host (when pass_host = "rewrite")
    #[serde(default)]
    pub upstream_host: Option<String>,

    /// Active health check configuration
    #[serde(default)]
    pub checks: Option<HealthCheckConfig>,

    /// Service discovery
    #[serde(default)]
    pub discovery: Option<DiscoveryConfig>,

    /// TLS settings for upstream
    #[serde(default)]
    pub tls: Option<UpstreamTls>,

    /// Labels
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Creation timestamp
    #[serde(default)]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Update timestamp
    #[serde(default)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalancerType {
    /// Weighted round robin
    Roundrobin,
    /// Consistent hashing
    Chash,
    /// Exponentially Weighted Moving Average
    Ewma,
    /// Least connections
    LeastConn,
}

impl Default for LoadBalancerType {
    fn default() -> Self {
        Self::Roundrobin
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PassHostMode {
    /// Pass client Host header as-is
    Pass,
    /// Use upstream node's host
    Node,
    /// Rewrite with upstream_host
    Rewrite,
}

impl Default for PassHostMode {
    fn default() -> Self {
        Self::Pass
    }
}

/// Health check configuration for upstreams.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Active health check
    #[serde(default)]
    pub active: Option<ActiveHealthCheck>,

    /// Passive health check (circuit breaker)
    #[serde(default)]
    pub passive: Option<PassiveHealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveHealthCheck {
    /// Check type: "http", "https", "tcp"
    #[serde(default = "default_check_type")]
    pub r#type: String,

    /// Check interval (seconds)
    #[serde(default = "default_check_interval")]
    pub interval: u64,

    /// Timeout for each check (seconds)
    #[serde(default = "default_check_timeout")]
    pub timeout: f64,

    /// HTTP path to check
    #[serde(default = "default_check_path")]
    pub http_path: String,

    /// Expected HTTP status codes for healthy
    #[serde(default = "default_healthy_statuses")]
    pub healthy_statuses: Vec<u16>,

    /// Number of consecutive successes to mark healthy
    #[serde(default = "default_healthy_successes")]
    pub healthy_successes: u32,

    /// Number of consecutive failures to mark unhealthy
    #[serde(default = "default_unhealthy_failures")]
    pub unhealthy_failures: u32,

    /// Request headers
    #[serde(default)]
    pub req_headers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassiveHealthCheck {
    /// Healthy configuration
    #[serde(default)]
    pub healthy: PassiveHealthyConfig,

    /// Unhealthy configuration
    #[serde(default)]
    pub unhealthy: PassiveUnhealthyConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PassiveHealthyConfig {
    /// HTTP codes considered healthy (default: 200-399)
    #[serde(default = "default_passive_healthy_statuses")]
    pub http_statuses: Vec<u16>,

    /// Successes to restore from unhealthy
    #[serde(default = "default_passive_successes")]
    pub successes: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PassiveUnhealthyConfig {
    /// HTTP codes considered unhealthy
    #[serde(default = "default_passive_unhealthy_statuses")]
    pub http_statuses: Vec<u16>,

    /// Failures to mark unhealthy
    #[serde(default = "default_passive_failures")]
    pub failures: u32,

    /// TCP failures threshold
    #[serde(default = "default_passive_tcp_failures")]
    pub tcp_failures: u32,

    /// Timeout count threshold
    #[serde(default = "default_passive_timeouts")]
    pub timeouts: u32,
}

/// Service discovery configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Discovery type: "dns", "consul", "eureka", "nacos"
    pub r#type: String,

    /// Service name to discover
    pub service_name: String,

    /// Additional discovery parameters
    #[serde(default)]
    pub args: HashMap<String, String>,
}

/// Upstream TLS verification settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamTls {
    /// Verify upstream SSL certificate
    #[serde(default)]
    pub verify: bool,

    /// CA certificate path
    #[serde(default)]
    pub ca_cert: Option<String>,

    /// Client certificate for mTLS
    #[serde(default)]
    pub client_cert: Option<String>,

    /// Client key for mTLS
    #[serde(default)]
    pub client_key: Option<String>,
}

// Defaults
fn default_lb_type() -> LoadBalancerType {
    LoadBalancerType::Roundrobin
}

fn default_retries() -> u32 {
    1
}

fn default_scheme() -> String {
    "http".to_string()
}

fn default_pass_host() -> PassHostMode {
    PassHostMode::Pass
}

fn default_check_type() -> String {
    "http".to_string()
}

fn default_check_interval() -> u64 {
    5
}

fn default_check_timeout() -> f64 {
    1.0
}

fn default_check_path() -> String {
    "/".to_string()
}

fn default_healthy_statuses() -> Vec<u16> {
    vec![200, 302]
}

fn default_healthy_successes() -> u32 {
    2
}

fn default_unhealthy_failures() -> u32 {
    3
}

fn default_passive_healthy_statuses() -> Vec<u16> {
    (200..=399).collect()
}

fn default_passive_successes() -> u32 {
    5
}

fn default_passive_unhealthy_statuses() -> Vec<u16> {
    vec![500, 502, 503, 504]
}

fn default_passive_failures() -> u32 {
    5
}

fn default_passive_tcp_failures() -> u32 {
    2
}

fn default_passive_timeouts() -> u32 {
    7
}
