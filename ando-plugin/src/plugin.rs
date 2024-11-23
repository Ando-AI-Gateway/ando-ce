use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use ando_core::consumer::Consumer;

/// Plugin execution phases, matching APISIX's lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    /// Modify request before routing takes place
    Rewrite = 0,
    /// Authentication, authorization, rate limiting
    Access = 1,
    /// Just before proxying to upstream
    BeforeProxy = 2,
    /// Modify response headers from upstream
    HeaderFilter = 3,
    /// Modify response body from upstream
    BodyFilter = 4,
    /// Post-response logging (non-blocking)
    Log = 5,
}

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::Rewrite => "rewrite",
            Phase::Access => "access",
            Phase::BeforeProxy => "before_proxy",
            Phase::HeaderFilter => "header_filter",
            Phase::BodyFilter => "body_filter",
            Phase::Log => "log",
        }
    }

    pub fn all() -> &'static [Phase] {
        &[
            Phase::Rewrite,
            Phase::Access,
            Phase::BeforeProxy,
            Phase::HeaderFilter,
            Phase::BodyFilter,
            Phase::Log,
        ]
    }
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Result of plugin execution.
#[derive(Debug)]
pub enum PluginResult {
    /// Continue to the next plugin / phase
    Continue,

    /// Short-circuit with a response (e.g., 401, 403, 429)
    Response {
        status: u16,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
    },

    /// Error during plugin execution
    Error(String),
}

/// Mutable context passed through the plugin pipeline for each request.
///
/// This is the Rust-side representation of the request/response data that
/// the Lua PDK also exposes.
pub struct PluginContext {
    // --- Request data ---
    pub request_method: String,
    pub request_uri: String,
    pub request_path: String,
    pub request_query: String,
    pub request_headers: HashMap<String, String>,
    pub request_body: Option<Vec<u8>>,

    /// Path parameters from router matching
    pub path_params: HashMap<String, String>,

    /// Client IP address
    pub client_ip: String,

    // --- Response data (populated after upstream response) ---
    pub response_status: Option<u16>,
    pub response_headers: HashMap<String, String>,
    pub response_body: Option<Vec<u8>>,

    // --- Plugin data ---
    /// Shared context between plugins (key-value store)
    pub vars: HashMap<String, Value>,

    /// Consumer identified by auth plugins
    pub consumer: Option<ando_core::consumer::Consumer>,

    /// Route matched
    pub route_id: String,

    /// Service ID (if any)
    pub service_id: Option<String>,

    // --- Timing ---
    pub request_start: std::time::Instant,

    // --- Upstream selection ---
    pub upstream_addr: Option<String>,

    /// Snapshot of consumers for auth plugins to validate against
    /// (populated by the proxy before the plugin pipeline runs)
    pub consumers: HashMap<String, Consumer>,
}

impl PluginContext {
    pub fn new(
        method: String,
        uri: String,
        headers: HashMap<String, String>,
        client_ip: String,
        route_id: String,
    ) -> Self {
        // Parse path and query from URI
        let (path, query) = match uri.find('?') {
            Some(pos) => (uri[..pos].to_string(), uri[pos + 1..].to_string()),
            None => (uri.clone(), String::new()),
        };

        Self {
            request_method: method,
            request_uri: uri,
            request_path: path,
            request_query: query,
            request_headers: headers,
            request_body: None,
            path_params: HashMap::new(),
            client_ip,
            response_status: None,
            response_headers: HashMap::new(),
            response_body: None,
            vars: HashMap::new(),
            consumer: None,
            route_id,
            service_id: None,
            request_start: std::time::Instant::now(),
            upstream_addr: None,
            consumers: HashMap::new(),
        }
    }

    /// Get a request header (case-insensitive).
    pub fn get_header(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        self.request_headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }

    /// Set a request header.
    pub fn set_header(&mut self, name: String, value: String) {
        self.request_headers.insert(name, value);
    }

    /// Remove a request header.
    pub fn remove_header(&mut self, name: &str) {
        let lower = name.to_lowercase();
        self.request_headers
            .retain(|k, _| k.to_lowercase() != lower);
    }

    /// Set a response header.
    pub fn set_response_header(&mut self, name: String, value: String) {
        self.response_headers.insert(name, value);
    }

    /// Get elapsed time since request start.
    pub fn elapsed_ms(&self) -> f64 {
        self.request_start.elapsed().as_secs_f64() * 1000.0
    }

    /// Set a context variable (shared between plugins).
    pub fn set_var(&mut self, key: String, value: Value) {
        self.vars.insert(key, value);
    }

    /// Get a context variable.
    pub fn get_var(&self, key: &str) -> Option<&Value> {
        self.vars.get(key)
    }
}

/// The core Plugin trait. All plugins (Rust built-in or Lua wrappers) implement this.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin name (must be unique)
    fn name(&self) -> &str;

    /// Plugin priority (higher = executed first within a phase)
    fn priority(&self) -> i32 {
        0
    }

    /// Which phases this plugin participates in
    fn phases(&self) -> Vec<Phase> {
        vec![Phase::Access]
    }

    /// Validate plugin configuration
    fn check_schema(&self, config: &Value) -> anyhow::Result<()> {
        let _ = config;
        Ok(())
    }

    /// Execute the plugin at the given phase
    async fn execute(
        &self,
        phase: Phase,
        ctx: &mut PluginContext,
        config: &Value,
    ) -> PluginResult;
}

/// A plugin instance bound to a specific configuration.
pub struct PluginInstance {
    pub plugin: Arc<dyn Plugin>,
    pub config: Value,
    pub name: String,
}

impl PluginInstance {
    pub fn new(plugin: Arc<dyn Plugin>, config: Value) -> Self {
        let name = plugin.name().to_string();
        Self {
            plugin,
            config,
            name,
        }
    }
}
