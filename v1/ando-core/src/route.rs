use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A Route defines how incoming requests are matched and handled.
/// Modeled after APISIX Route object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Unique route identifier
    pub id: String,

    /// Human-readable name
    #[serde(default)]
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// URI path pattern (supports exact, prefix with `*`, and parametric `:param`)
    pub uri: String,

    /// Additional URI patterns (OR match)
    #[serde(default)]
    pub uris: Vec<String>,

    /// Allowed HTTP methods (empty = all methods)
    #[serde(default)]
    pub methods: Vec<HttpMethod>,

    /// Host header matching
    #[serde(default)]
    pub host: Option<String>,

    /// Additional host patterns (OR match)
    #[serde(default)]
    pub hosts: Vec<String>,

    /// Remote address CIDR matching
    #[serde(default)]
    pub remote_addrs: Vec<String>,

    /// Request header matching (all must match)
    #[serde(default)]
    pub vars: Vec<RouteVar>,

    /// Priority (higher = matched first, default 0)
    #[serde(default)]
    pub priority: i32,

    /// Whether this route is enabled
    #[serde(default = "default_enabled")]
    pub enable: bool,

    /// Upstream configuration (inline or reference)
    #[serde(default)]
    pub upstream: Option<InlineUpstream>,

    /// Reference to a named upstream
    #[serde(default)]
    pub upstream_id: Option<String>,

    /// Reference to a named service
    #[serde(default)]
    pub service_id: Option<String>,

    /// Plugin chain configuration: plugin_name -> config
    #[serde(default)]
    pub plugins: HashMap<String, serde_json::Value>,

    /// Plugin config reference ID
    #[serde(default)]
    pub plugin_config_id: Option<String>,

    /// Labels (metadata)
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Route status (1 = enabled, 0 = disabled)
    #[serde(default = "default_status")]
    pub status: u8,

    /// Timeout overrides for this route
    #[serde(default)]
    pub timeout: Option<TimeoutConfig>,

    /// Creation timestamp
    #[serde(default)]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Last update timestamp
    #[serde(default)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// HTTP methods supported by routes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Connect,
    Trace,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Connect => "CONNECT",
            HttpMethod::Trace => "TRACE",
        }
    }
}

/// Route variable condition for advanced matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteVar {
    /// Variable to check (e.g., "http_x_api_version", "arg_key")
    pub var: String,

    /// Operator: "==", "!=", "~=", ">=", "<=", ">", "<", "in", "not_in"
    pub operator: String,

    /// Value to compare against
    pub value: serde_json::Value,
}

/// Inline upstream definition within a route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineUpstream {
    /// Upstream type: "roundrobin", "chash", "ewma", "least_conn"
    #[serde(default = "default_upstream_type")]
    pub r#type: String,

    /// Backend nodes: address -> weight
    #[serde(default)]
    pub nodes: HashMap<String, u32>,

    /// Timeout overrides
    #[serde(default)]
    pub timeout: Option<TimeoutConfig>,

    /// Number of retries
    #[serde(default = "default_retries")]
    pub retries: u32,

    /// Retry timeout
    #[serde(default)]
    pub retry_timeout: Option<u64>,

    /// Pass host mode: "pass", "node", "rewrite"
    #[serde(default = "default_pass_host")]
    pub pass_host: String,

    /// Upstream host header (when pass_host = "rewrite")
    #[serde(default)]
    pub upstream_host: Option<String>,

    /// Scheme: "http", "https", "grpc", "grpcs"
    #[serde(default = "default_scheme")]
    pub scheme: String,
}

/// Timeout configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Connect timeout (seconds)
    #[serde(default = "default_timeout")]
    pub connect: f64,

    /// Send timeout (seconds)
    #[serde(default = "default_timeout")]
    pub send: f64,

    /// Read timeout (seconds)
    #[serde(default = "default_timeout")]
    pub read: f64,
}

impl Route {
    /// Check if a given HTTP method is allowed by this route.
    pub fn method_allowed(&self, method: &str) -> bool {
        if self.methods.is_empty() {
            return true;
        }
        self.methods.iter().any(|m| m.as_str() == method)
    }

    /// Check if the route is active.
    pub fn is_active(&self) -> bool {
        self.enable && self.status == 1
    }

    /// Get all URI patterns for this route.
    pub fn all_uris(&self) -> Vec<&str> {
        let mut uris = vec![self.uri.as_str()];
        for u in &self.uris {
            uris.push(u.as_str());
        }
        uris
    }

    /// Get all hosts for this route.
    pub fn all_hosts(&self) -> Vec<&str> {
        let mut hosts = Vec::new();
        if let Some(ref h) = self.host {
            hosts.push(h.as_str());
        }
        for h in &self.hosts {
            hosts.push(h.as_str());
        }
        hosts
    }
}

// Defaults

fn default_enabled() -> bool {
    true
}

fn default_status() -> u8 {
    1
}

fn default_upstream_type() -> String {
    "roundrobin".to_string()
}

fn default_retries() -> u32 {
    1
}

fn default_pass_host() -> String {
    "pass".to_string()
}

fn default_scheme() -> String {
    "http".to_string()
}

fn default_timeout() -> f64 {
    6.0
}
