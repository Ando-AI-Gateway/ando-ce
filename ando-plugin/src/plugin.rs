use std::collections::HashMap;

/// Plugin execution phases — APISIX-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    Rewrite,
    Access,
    BeforeProxy,
    HeaderFilter,
    BodyFilter,
    Log,
}

/// Result of plugin execution.
pub enum PluginResult {
    /// Continue to next plugin / proxy upstream.
    Continue,
    /// Short-circuit with an HTTP response (e.g., 401, 403, 429).
    Response {
        status: u16,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    },
}

/// Context passed through the plugin pipeline for a single request.
///
/// v2 design: Stack-allocated where possible. No Box, no Arc on the hot path.
/// Total size target: ≤128 bytes for the core struct (cache-line friendly).
pub struct PluginContext {
    /// Route ID.
    pub route_id: String,
    /// Client IP.
    pub client_ip: String,
    /// HTTP method.
    pub method: String,
    /// Request URI.
    pub uri: String,
    /// Request headers (lowercase keys).
    pub request_headers: HashMap<String, String>,
    /// Response status (set by upstream or plugin).
    pub response_status: Option<u16>,
    /// Response headers to add/modify.
    pub response_headers: HashMap<String, String>,
    /// Matched consumer username (set by auth plugins).
    pub consumer: Option<String>,
    /// Arbitrary plugin context data.
    pub vars: HashMap<String, serde_json::Value>,
}

impl PluginContext {
    pub fn new(
        route_id: String,
        client_ip: String,
        method: String,
        uri: String,
        request_headers: HashMap<String, String>,
    ) -> Self {
        Self {
            route_id,
            client_ip,
            method,
            uri,
            request_headers,
            response_status: None,
            response_headers: HashMap::new(),
            consumer: None,
            vars: HashMap::new(),
        }
    }

    /// Get a request header value.
    #[inline]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.request_headers.get(name).map(|s| s.as_str())
    }
}

/// The Plugin trait — implemented by all plugins (Rust native).
///
/// v2 design: Synchronous execution by default. Plugins run on the
/// monoio worker thread — no async overhead for simple plugins.
/// Complex plugins that need I/O can use monoio's async internally.
pub trait Plugin: Send + Sync {
    /// Plugin name (must be unique).
    fn name(&self) -> &str;

    /// Plugin priority (higher = runs first within a phase).
    fn priority(&self) -> i32 { 0 }

    /// Which phases this plugin participates in.
    fn phases(&self) -> &[Phase];

    /// Create a configured instance from JSON config.
    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>>;
}

/// A configured plugin instance bound to a specific route.
pub trait PluginInstance: Send + Sync {
    /// Plugin name.
    fn name(&self) -> &str;

    /// Priority.
    fn priority(&self) -> i32 { 0 }

    /// Execute the rewrite phase.
    fn rewrite(&self, _ctx: &mut PluginContext) -> PluginResult { PluginResult::Continue }

    /// Execute the access phase (auth, rate limiting, etc.).
    fn access(&self, _ctx: &mut PluginContext) -> PluginResult { PluginResult::Continue }

    /// Execute before proxying upstream.
    fn before_proxy(&self, _ctx: &mut PluginContext) -> PluginResult { PluginResult::Continue }

    /// Execute header filter phase.
    fn header_filter(&self, _ctx: &mut PluginContext) -> PluginResult { PluginResult::Continue }

    /// Execute body filter phase.
    fn body_filter(&self, _ctx: &mut PluginContext, _body: &mut Vec<u8>) -> PluginResult {
        PluginResult::Continue
    }

    /// Execute log phase (fire-and-forget).
    fn log(&self, _ctx: &PluginContext) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_ctx(headers: Vec<(&str, &str)>) -> PluginContext {
        let map: HashMap<String, String> = headers
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        PluginContext::new(
            "route1".into(),
            "127.0.0.1".into(),
            "GET".into(),
            "/api/test".into(),
            map,
        )
    }

    #[test]
    fn test_context_fields() {
        let ctx = make_ctx(vec![("apikey", "my-key"), ("content-type", "application/json")]);
        assert_eq!(ctx.route_id, "route1");
        assert_eq!(ctx.client_ip, "127.0.0.1");
        assert_eq!(ctx.method, "GET");
        assert_eq!(ctx.uri, "/api/test");
        assert!(ctx.response_status.is_none());
        assert!(ctx.consumer.is_none());
        assert!(ctx.vars.is_empty());
        assert!(ctx.response_headers.is_empty());
    }

    #[test]
    fn test_get_header_present() {
        let ctx = make_ctx(vec![("apikey", "secret")]);
        assert_eq!(ctx.get_header("apikey"), Some("secret"));
    }

    #[test]
    fn test_get_header_absent() {
        let ctx = make_ctx(vec![]);
        assert_eq!(ctx.get_header("apikey"), None);
        assert_eq!(ctx.get_header("x-token"), None);
    }

    #[test]
    fn test_context_vars_mutable() {
        let mut ctx = make_ctx(vec![]);
        ctx.vars.insert("_key".into(), serde_json::json!("value"));
        assert_eq!(ctx.vars["_key"], "value");
    }

    #[test]
    fn test_context_consumer_mutable() {
        let mut ctx = make_ctx(vec![]);
        ctx.consumer = Some("alice".into());
        assert_eq!(ctx.consumer.as_deref(), Some("alice"));
    }

    #[test]
    fn test_plugin_result_continue() {
        let result = PluginResult::Continue;
        assert!(matches!(result, PluginResult::Continue));
    }

    #[test]
    fn test_plugin_result_response_401() {
        let result = PluginResult::Response {
            status: 401,
            headers: vec![("www-authenticate".into(), "Bearer".into())],
            body: Some(b"Unauthorized".to_vec()),
        };
        if let PluginResult::Response { status, body, .. } = result {
            assert_eq!(status, 401);
            assert_eq!(body.unwrap(), b"Unauthorized");
        } else {
            panic!("Expected Response variant");
        }
    }

    #[test]
    fn test_plugin_result_response_no_body() {
        let result = PluginResult::Response {
            status: 429,
            headers: vec![],
            body: None,
        };
        if let PluginResult::Response { status, body, .. } = result {
            assert_eq!(status, 429);
            assert!(body.is_none());
        } else {
            panic!("Expected Response variant");
        }
    }

    #[test]
    fn test_phase_equality() {
        assert_eq!(Phase::Access, Phase::Access);
        assert_ne!(Phase::Access, Phase::Rewrite);
        assert_ne!(Phase::HeaderFilter, Phase::BodyFilter);
    }
}
