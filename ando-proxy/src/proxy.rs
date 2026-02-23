use ando_core::config::GatewayConfig;
use ando_core::route::Route;
use ando_core::router::Router;
use ando_core::service::Service;
use ando_core::upstream::Upstream;
use ando_plugin::pipeline::PluginPipeline;
use ando_plugin::plugin::{Phase, PluginContext, PluginResult};
use ando_plugin::registry::PluginRegistry;
use ando_store::cache::ConfigCache;
use monoio::net::TcpStream;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

// ── Pre-built static error responses (zero heap alloc) ────────

pub const RESP_404: &[u8] =
    b"HTTP/1.1 404 Not Found\r\ncontent-type: application/json\r\ncontent-length: 41\r\nconnection: keep-alive\r\n\r\n{\"error\":\"no route matched\",\"status\":404}";

pub const RESP_401_INVALID: &[u8] =
    b"HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: 40\r\nconnection: keep-alive\r\n\r\n{\"error\":\"Invalid API key\",\"status\":401}";

pub const RESP_502: &[u8] =
    b"HTTP/1.1 502 Bad Gateway\r\ncontent-type: application/json\r\ncontent-length: 39\r\nconnection: keep-alive\r\n\r\n{\"error\":\"upstream error\",\"status\":502}";

// ── ProxyWorker ───────────────────────────────────────────────

/// Per-worker proxy state. Created ONCE per thread, reused across
/// all connections via Rc<RefCell<ProxyWorker>>.
///
/// All caches are plain HashMaps — zero atomics on hot path.
/// DashMap is only touched during config rebuild (cold path).
pub struct ProxyWorker {
    /// Current frozen router.
    router: Arc<Router>,
    /// Router version for cache invalidation.
    router_version: u64,

    // ── Thread-local caches (rebuilt on version change) ──
    pipeline_cache: HashMap<String, Arc<PluginPipeline>>,

    // ── Snapshots from DashMap (cold path only) ──
    upstreams: HashMap<String, Upstream>,
    services: HashMap<String, Service>,
    consumer_keys: HashMap<String, String>,

    // ── Shared immutable ──
    plugin_registry: Arc<PluginRegistry>,
    config_cache: ConfigCache,
    #[allow(dead_code)]
    config: Arc<GatewayConfig>,
}

impl ProxyWorker {
    pub fn new(
        router: Arc<Router>,
        plugin_registry: Arc<PluginRegistry>,
        config_cache: ConfigCache,
        config: Arc<GatewayConfig>,
    ) -> Self {
        let mut worker = Self {
            router_version: router.version(),
            router,
            pipeline_cache: HashMap::with_capacity(64),
            upstreams: HashMap::new(),
            services: HashMap::new(),
            consumer_keys: HashMap::new(),
            plugin_registry,
            config_cache,
            config,
        };
        worker.snapshot_from_cache();
        worker
    }

    /// Check for config updates. Called once per accept loop iteration.
    #[inline]
    pub fn maybe_update_router(&mut self, new_router: Arc<Router>) {
        let v = new_router.version();
        if v != self.router_version {
            self.router = new_router;
            self.router_version = v;
            self.pipeline_cache.clear();
            self.snapshot_from_cache();
        }
    }

    /// Cold path: copy DashMap state into thread-local HashMaps.
    fn snapshot_from_cache(&mut self) {
        self.upstreams.clear();
        for entry in self.config_cache.upstreams.iter() {
            self.upstreams.insert(entry.key().clone(), entry.value().clone());
        }
        self.services.clear();
        for entry in self.config_cache.services.iter() {
            self.services.insert(entry.key().clone(), entry.value().clone());
        }
        self.consumer_keys.clear();
        for entry in self.config_cache.consumer_key_index.iter() {
            self.consumer_keys.insert(entry.key().clone(), entry.value().clone());
        }
    }

    /// Collect all unique upstream addresses from config (for pool pre-warming).
    pub fn upstream_addresses(&self) -> Vec<String> {
        let mut addrs = Vec::new();
        for ups in self.upstreams.values() {
            for addr in ups.nodes.keys() {
                if !addrs.contains(addr) {
                    addrs.push(addr.clone());
                }
            }
        }
        // Also check routes with inline upstreams
        for route in self.router.routes().values() {
            if let Some(ref ups) = route.upstream {
                for addr in ups.nodes.keys() {
                    if !addrs.contains(addr) {
                        addrs.push(addr.clone());
                    }
                }
            }
        }
        addrs
    }

    /// Hot path: process request. Returns what to do next.
    ///
    /// Takes &str header references (zero-copy from read buffer).
    /// No DashMap access. No unnecessary allocations.
    #[inline]
    pub fn handle_request(
        &mut self,
        method: &str,
        path: &str,
        host: Option<&str>,
        headers: &[(&str, &str)],
        client_ip: &str,
    ) -> RequestResult {
        // ── Route match — extract data immediately, release borrow ──
        let (route_id, has_plugins, upstream_addr) = {
            let route = match self.router.match_route(method, path, host) {
                Some(r) => r,
                None => return RequestResult::Static(RESP_404),
            };

            let id = route.id.clone();
            let has_plugins = !route.plugins.is_empty()
                || route.plugin_config_id.is_some()
                || route.service_id.is_some();
            let addr = self.resolve_upstream(route);
            (id, has_plugins, addr)
        };
        // immutable borrow of self.router is now released

        // ── FAST PATH: no plugins → proxy directly ──
        if !has_plugins {
            return RequestResult::Proxy { upstream_addr };
        }

        // ── SLOW PATH: plugin pipeline ──
        let pipeline = self.get_or_build_pipeline(&route_id);

        // Build PluginContext (only for routes WITH plugins)
        let header_map: HashMap<String, String> = headers
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.to_string()))
            .collect();

        let mut ctx = PluginContext::new(
            route_id.clone(),
            client_ip.to_string(),
            method.to_string(),
            path.to_string(),
            header_map,
        );

        // Execute Rewrite + Access phases
        for phase in &[Phase::Rewrite, Phase::Access] {
            match pipeline.execute_phase(*phase, &mut ctx) {
                PluginResult::Continue => {}
                PluginResult::Response { status, headers, body } => {
                    return RequestResult::PluginResponse {
                        status,
                        headers,
                        body: body.unwrap_or_default(),
                    };
                }
            }
        }

        // Consumer validation (key-auth)
        if pipeline.has_auth_plugins()
            && let Some(key) = ctx.vars.get("_key_auth_key").and_then(|v| v.as_str())
        {
            match self.consumer_keys.get(key) {
                Some(username) => ctx.consumer = Some(username.clone()),
                None => return RequestResult::Static(RESP_401_INVALID),
            }
        }

        // Before proxy phase
        match pipeline.execute_phase(Phase::BeforeProxy, &mut ctx) {
            PluginResult::Continue => {}
            PluginResult::Response { status, headers, body } => {
                return RequestResult::PluginResponse {
                    status,
                    headers,
                    body: body.unwrap_or_default(),
                };
            }
        }

        RequestResult::Proxy { upstream_addr }
    }

    /// Resolve upstream address from local snapshot (never DashMap).
    fn resolve_upstream(&self, route: &Route) -> String {
        if let Some(ref ups) = route.upstream
            && let Some(addr) = ups.first_node()
        {
            return addr.to_string();
        }
        if let Some(ref id) = route.upstream_id
            && let Some(ups) = self.upstreams.get(id)
            && let Some(addr) = ups.first_node()
        {
            return addr.to_string();
        }
        if let Some(ref svc_id) = route.service_id
            && let Some(svc) = self.services.get(svc_id)
        {
            if let Some(ref ups) = svc.upstream
                && let Some(addr) = ups.first_node()
            {
                return addr.to_string();
            }
            if let Some(ref ups_id) = svc.upstream_id
                && let Some(ups) = self.upstreams.get(ups_id)
                && let Some(addr) = ups.first_node()
            {
                return addr.to_string();
            }
        }
        "127.0.0.1:80".to_string()
    }

    fn get_or_build_pipeline(&mut self, route_id: &str) -> Arc<PluginPipeline> {
        if let Some(cached) = self.pipeline_cache.get(route_id) {
            return Arc::clone(cached);
        }

        let route = self.router.get_route(route_id);
        let mut merged: HashMap<String, serde_json::Value> = HashMap::new();
        let mut has_auth = false;

        if let Some(route) = route {
            if let Some(ref svc_id) = route.service_id
                && let Some(svc) = self.services.get(svc_id)
            {
                for (name, config) in &svc.plugins {
                    merged.insert(name.clone(), config.clone());
                }
            }
            for (name, config) in &route.plugins {
                merged.insert(name.clone(), config.clone());
            }
        }

        let mut instances: Vec<Arc<dyn ando_plugin::plugin::PluginInstance>> = Vec::new();
        for (name, config) in &merged {
            if matches!(name.as_str(), "key-auth" | "jwt-auth" | "basic-auth") {
                has_auth = true;
            }
            if let Some(factory) = self.plugin_registry.get(name)
                && let Ok(inst) = factory.configure(config)
            {
                instances.push(Arc::from(inst));
            }
        }

        let pipeline = Arc::new(PluginPipeline::build(instances, has_auth));
        self.pipeline_cache.insert(route_id.to_string(), Arc::clone(&pipeline));
        pipeline
    }
}

// ── Request result ────────────────────────────────────────────

#[derive(Debug)]
pub enum RequestResult {
    /// Proxy to upstream at this address.
    Proxy { upstream_addr: String },
    /// Send a pre-built static response (zero alloc).
    Static(&'static [u8]),
    /// Send a plugin-generated response.
    PluginResponse {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
}

// ── Connection pool ───────────────────────────────────────────

/// Thread-local upstream connection pool.
/// Avoids TCP handshake on every request (saves ~0.5-2ms RTT).
///
/// Pre-warmed at startup: each worker opens N connections to every
/// known upstream before accepting any traffic.
pub struct ConnPool {
    pools: HashMap<String, VecDeque<TcpStream>>,
    max_idle: usize,
}

impl ConnPool {
    pub fn new(max_idle_per_host: usize) -> Self {
        Self {
            pools: HashMap::with_capacity(16),
            max_idle: max_idle_per_host,
        }
    }

    #[inline]
    pub fn take(&mut self, addr: &str) -> Option<TcpStream> {
        self.pools.get_mut(addr).and_then(|q| q.pop_front())
    }

    #[inline]
    pub fn put(&mut self, addr: String, stream: TcpStream) {
        let queue = self.pools.entry(addr).or_insert_with(|| VecDeque::with_capacity(self.max_idle));
        if queue.len() < self.max_idle {
            queue.push_back(stream);
        }
        // else: drop stream (closes fd)
    }

    /// Pre-warm connection pool: open `count` connections to each addr.
    /// Called once at worker startup, before accepting any traffic.
    pub async fn warm(&mut self, addrs: &[String], count: usize) {
        for addr in addrs {
            let target = count.min(self.max_idle);
            let queue = self.pools.entry(addr.clone()).or_insert_with(|| VecDeque::with_capacity(target));
            for _ in 0..target {
                match TcpStream::connect(addr.as_str()).await {
                    Ok(stream) => {
                        // Set TCP_NODELAY on pooled connections
                        let _ = stream.set_nodelay(true);
                        queue.push_back(stream);
                    }
                    Err(e) => {
                        tracing::warn!(addr = %addr, error = %e, "Pool pre-warm connect failed");
                        break; // upstream not yet up — stop trying this addr
                    }
                }
            }
            if !queue.is_empty() {
                tracing::info!(addr = %addr, conns = queue.len(), "Pool pre-warmed");
            }
        }
    }
}

// ── Response building helpers ─────────────────────────────────

/// Build HTTP response into a buffer (no format! overhead).
pub fn build_response(buf: &mut Vec<u8>, status: u16, headers: &[(String, String)], body: &[u8]) {
    buf.clear();
    buf.extend_from_slice(b"HTTP/1.1 ");
    let mut itoa_buf = itoa::Buffer::new();
    buf.extend_from_slice(itoa_buf.format(status).as_bytes());
    buf.push(b' ');
    buf.extend_from_slice(status_text(status).as_bytes());
    buf.extend_from_slice(b"\r\ncontent-length: ");
    buf.extend_from_slice(itoa_buf.format(body.len()).as_bytes());
    buf.extend_from_slice(b"\r\nconnection: keep-alive\r\n");
    for (k, v) in headers {
        buf.extend_from_slice(k.as_bytes());
        buf.extend_from_slice(b": ");
        buf.extend_from_slice(v.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    buf.extend_from_slice(b"\r\n");
    buf.extend_from_slice(body);
}

/// Build upstream HTTP request into a buffer. Zero-copy from &str refs.
pub fn build_upstream_request(
    buf: &mut Vec<u8>,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) {
    buf.clear();
    buf.extend_from_slice(method.as_bytes());
    buf.push(b' ');
    buf.extend_from_slice(path.as_bytes());
    buf.extend_from_slice(b" HTTP/1.1\r\n");
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("keep-alive")
            || name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("upgrade")
        {
            continue;
        }
        buf.extend_from_slice(name.as_bytes());
        buf.extend_from_slice(b": ");
        buf.extend_from_slice(value.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    buf.extend_from_slice(b"connection: keep-alive\r\n");
    if !body.is_empty() {
        buf.extend_from_slice(b"content-length: ");
        let mut itoa_buf = itoa::Buffer::new();
        buf.extend_from_slice(itoa_buf.format(body.len()).as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    buf.extend_from_slice(b"\r\n");
    if !body.is_empty() {
        buf.extend_from_slice(body);
    }
}

pub fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ando_core::config::GatewayConfig;
    use ando_core::consumer::Consumer;
    use ando_core::route::Route;
    use ando_core::router::Router;
    use ando_plugin::registry::PluginRegistry;
    use ando_store::cache::ConfigCache;
    use std::collections::HashMap;
    use std::sync::Arc;

    // ── Helpers ──────────────────────────────────────────────────

    fn make_worker_with_registry(routes: Vec<Route>, registry: PluginRegistry, cache: ConfigCache) -> ProxyWorker {
        let router = Arc::new(Router::build(routes, 1).unwrap());
        let config = Arc::new(GatewayConfig::default());
        ProxyWorker::new(router, Arc::new(registry), cache, config)
    }

    fn make_worker(routes: Vec<Route>) -> ProxyWorker {
        make_worker_with_registry(routes, PluginRegistry::new(), ConfigCache::new())
    }

    fn simple_route(id: &str, uri: &str, upstream_addr: &str) -> Route {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "uri": uri,
            "status": 1,
            "upstream": { "nodes": { upstream_addr: 1 }, "type": "roundrobin" }
        })).unwrap()
    }

    fn route_with_key_auth(id: &str, uri: &str, upstream_addr: &str) -> Route {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "uri": uri,
            "status": 1,
            "plugins": { "key-auth": {} },
            "upstream": { "nodes": { upstream_addr: 1 }, "type": "roundrobin" }
        })).unwrap()
    }

    // ── status_text ──────────────────────────────────────────────

    #[test]
    fn status_text_known_codes() {
        assert_eq!(status_text(200), "OK");
        assert_eq!(status_text(201), "Created");
        assert_eq!(status_text(204), "No Content");
        assert_eq!(status_text(301), "Moved Permanently");
        assert_eq!(status_text(302), "Found");
        assert_eq!(status_text(400), "Bad Request");
        assert_eq!(status_text(401), "Unauthorized");
        assert_eq!(status_text(403), "Forbidden");
        assert_eq!(status_text(404), "Not Found");
        assert_eq!(status_text(429), "Too Many Requests");
        assert_eq!(status_text(500), "Internal Server Error");
        assert_eq!(status_text(502), "Bad Gateway");
        assert_eq!(status_text(503), "Service Unavailable");
        assert_eq!(status_text(504), "Gateway Timeout");
    }

    #[test]
    fn status_text_unknown_code_returns_unknown() {
        assert_eq!(status_text(999), "Unknown");
        assert_eq!(status_text(0), "Unknown");
    }

    // ── build_response ───────────────────────────────────────────

    #[test]
    fn build_response_status_line_and_body() {
        let mut buf = Vec::new();
        build_response(&mut buf, 200, &[], b"hello");
        let text = String::from_utf8(buf).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "must start with status line");
        assert!(text.contains("content-length: 5\r\n"), "must contain correct content-length");
        assert!(text.contains("connection: keep-alive\r\n"), "must contain keep-alive");
        assert!(text.ends_with("hello"), "body must be at end");
    }

    #[test]
    fn build_response_empty_body() {
        let mut buf = Vec::new();
        build_response(&mut buf, 204, &[], b"");
        let text = String::from_utf8(buf).unwrap();
        assert!(text.starts_with("HTTP/1.1 204 No Content\r\n"));
        assert!(text.contains("content-length: 0\r\n"));
    }

    #[test]
    fn build_response_custom_headers() {
        let mut buf = Vec::new();
        let headers = vec![
            ("x-custom".to_string(), "value1".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ];
        build_response(&mut buf, 200, &headers, b"{}");
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("x-custom: value1\r\n"));
        assert!(text.contains("content-type: application/json\r\n"));
    }

    #[test]
    fn build_response_clears_buffer_first() {
        let mut buf = b"stale data".to_vec();
        build_response(&mut buf, 200, &[], b"fresh");
        let text = String::from_utf8(buf).unwrap();
        assert!(!text.contains("stale data"));
        assert!(text.ends_with("fresh"));
    }

    // ── build_upstream_request ───────────────────────────────────

    #[test]
    fn build_upstream_request_basic_format() {
        let mut buf = Vec::new();
        build_upstream_request(&mut buf, "GET", "/api", &[], b"");
        let text = String::from_utf8(buf).unwrap();
        assert!(text.starts_with("GET /api HTTP/1.1\r\n"));
        assert!(text.contains("connection: keep-alive\r\n"));
    }

    #[test]
    fn build_upstream_request_filters_hop_by_hop_headers() {
        let mut buf = Vec::new();
        let headers = [
            ("connection", "close"),
            ("keep-alive", "timeout=5"),
            ("transfer-encoding", "chunked"),
            ("upgrade", "websocket"),
            ("x-forwarded-for", "1.2.3.4"),
        ];
        build_upstream_request(&mut buf, "POST", "/", &headers, b"");
        let text = String::from_utf8(buf).unwrap();
        // hop-by-hop must be removed
        assert!(!text.contains("transfer-encoding: chunked"));
        assert!(!text.contains("upgrade: websocket"));
        assert!(!text.contains("keep-alive: timeout=5"));
        // regular headers must pass through
        assert!(text.contains("x-forwarded-for: 1.2.3.4\r\n"));
    }

    #[test]
    fn build_upstream_request_adds_content_length_for_body() {
        let mut buf = Vec::new();
        build_upstream_request(&mut buf, "POST", "/", &[], b"body-data");
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("content-length: 9\r\n"));
        assert!(text.ends_with("body-data"));
    }

    // ── handle_request — route matching ─────────────────────────

    #[test]
    fn handle_request_unmatched_path_returns_404() {
        let mut w = make_worker(vec![simple_route("r1", "/api", "127.0.0.1:8080")]);
        let result = w.handle_request("GET", "/not-found", None, &[], "1.2.3.4");
        assert!(matches!(result, RequestResult::Static(RESP_404)));
    }

    #[test]
    fn handle_request_fast_path_proxy() {
        let mut w = make_worker(vec![simple_route("r1", "/api", "127.0.0.1:8080")]);
        let result = w.handle_request("GET", "/api", None, &[], "1.2.3.4");
        match result {
            RequestResult::Proxy { upstream_addr } => {
                assert_eq!(upstream_addr, "127.0.0.1:8080");
            }
            other => panic!("Expected Proxy, got {:?}", other),
        }
    }

    #[test]
    fn handle_request_disabled_route_returns_404() {
        let route: Route = serde_json::from_value(serde_json::json!({
            "id": "r1", "uri": "/disabled", "status": 0,
            "upstream": { "nodes": { "127.0.0.1:8080": 1 }, "type": "roundrobin" }
        })).unwrap();
        let mut w = make_worker(vec![route]);
        let result = w.handle_request("GET", "/disabled", None, &[], "1.2.3.4");
        assert!(matches!(result, RequestResult::Static(RESP_404)));
    }

    #[test]
    fn handle_request_wildcard_path_matches_subpaths() {
        let route: Route = serde_json::from_value(serde_json::json!({
            "id": "r1", "uri": "/api/*", "status": 1,
            "methods": ["GET"],
            "upstream": { "nodes": { "127.0.0.1:8080": 1 }, "type": "roundrobin" }
        })).unwrap();
        let mut w = make_worker(vec![route]);
        let result = w.handle_request("GET", "/api/users/list", None, &[], "1.2.3.4");
        assert!(matches!(result, RequestResult::Proxy { .. }));
    }

    #[test]
    fn handle_request_method_specific_route() {
        let route: Route = serde_json::from_value(serde_json::json!({
            "id": "r1", "uri": "/only-get",
            "methods": ["GET"], "status": 1,
            "upstream": { "nodes": { "127.0.0.1:8080": 1 }, "type": "roundrobin" }
        })).unwrap();
        let mut w = make_worker(vec![route]);
        assert!(matches!(w.handle_request("GET", "/only-get", None, &[], "x"), RequestResult::Proxy { .. }));
        assert!(matches!(w.handle_request("POST", "/only-get", None, &[], "x"), RequestResult::Static(RESP_404)));
    }

    // ── handle_request — key-auth plugin ────────────────────────

    #[test]
    fn handle_request_key_auth_missing_key_returns_plugin_401() {
        let mut registry = PluginRegistry::new();
        ando_plugins::register_all(&mut registry);
        let route = route_with_key_auth("r1", "/secure", "127.0.0.1:8080");
        let mut w = make_worker_with_registry(vec![route], registry, ConfigCache::new());

        // No apikey header → key-auth plugin returns 401
        let result = w.handle_request("GET", "/secure", None, &[], "1.2.3.4");
        match result {
            RequestResult::PluginResponse { status, .. } => assert_eq!(status, 401),
            other => panic!("Expected PluginResponse 401, got {:?}", other),
        }
    }

    #[test]
    fn handle_request_key_auth_invalid_key_returns_static_401() {
        let mut registry = PluginRegistry::new();
        ando_plugins::register_all(&mut registry);
        let route = route_with_key_auth("r1", "/secure", "127.0.0.1:8080");
        // Empty consumer store — key present in header but no consumer has it
        let mut w = make_worker_with_registry(vec![route], registry, ConfigCache::new());

        let result = w.handle_request("GET", "/secure", None, &[("apikey", "bad-key")], "1.2.3.4");
        assert!(matches!(result, RequestResult::Static(RESP_401_INVALID)),
            "wrong consumer key must return RESP_401_INVALID");
    }

    #[test]
    fn handle_request_key_auth_valid_key_proxies_request() {
        let mut registry = PluginRegistry::new();
        ando_plugins::register_all(&mut registry);
        let route = route_with_key_auth("r1", "/secure", "127.0.0.1:8080");

        // Add consumer with a known key
        let cache = ConfigCache::new();
        let mut plugins: HashMap<String, serde_json::Value> = HashMap::new();
        plugins.insert("key-auth".to_string(), serde_json::json!({ "key": "valid-key-123" }));
        cache.consumers.insert("alice".to_string(), Consumer {
            username: "alice".to_string(),
            plugins,
            desc: None,
            labels: HashMap::new(),
        });
        cache.rebuild_consumer_key_index();

        let mut w = make_worker_with_registry(vec![route], registry, cache);
        let result = w.handle_request("GET", "/secure", None, &[("apikey", "valid-key-123")], "1.2.3.4");
        assert!(matches!(result, RequestResult::Proxy { .. }),
            "valid consumer key must proxy the request");
    }

    // ── maybe_update_router ──────────────────────────────────────

    #[test]
    fn maybe_update_router_no_op_on_same_version() {
        let routes = vec![simple_route("r1", "/a", "127.0.0.1:8080")];
        let mut w = make_worker(routes);
        let old_version = w.router_version;
        let same_router = Arc::new(Router::build(vec![], old_version).unwrap());
        w.maybe_update_router(same_router);
        assert_eq!(w.router_version, old_version);
    }

    #[test]
    fn maybe_update_router_updates_on_new_version() {
        let routes = vec![simple_route("r1", "/a", "127.0.0.1:8080")];
        let mut w = make_worker(routes);
        let old_version = w.router_version;
        let new_router = Arc::new(Router::build(
            vec![simple_route("r2", "/b", "127.0.0.1:9090")],
            old_version + 1,
        ).unwrap());
        w.maybe_update_router(Arc::clone(&new_router));
        assert_eq!(w.router_version, old_version + 1);
        // New route should now match
        let result = w.handle_request("GET", "/b", None, &[], "x");
        assert!(matches!(result, RequestResult::Proxy { .. }));
    }

    // ── upstream_addresses ───────────────────────────────────────

    #[test]
    fn upstream_addresses_returns_inline_route_nodes() {
        let route = simple_route("r1", "/api", "10.0.0.1:8080");
        let w = make_worker(vec![route]);
        let addrs = w.upstream_addresses();
        assert!(addrs.contains(&"10.0.0.1:8080".to_string()));
    }

    // ── resolve_upstream: fallback to 127.0.0.1:80 ───────────────

    #[test]
    fn handle_request_no_upstream_falls_back_to_localhost() {
        let route: Route = serde_json::from_value(serde_json::json!({
            "id": "r1", "uri": "/no-ups", "status": 1
        })).unwrap();
        let mut w = make_worker(vec![route]);
        let result = w.handle_request("GET", "/no-ups", None, &[], "x");
        match result {
            RequestResult::Proxy { upstream_addr } => {
                assert_eq!(upstream_addr, "127.0.0.1:80",
                    "route with no upstream should fallback to 127.0.0.1:80");
            }
            other => panic!("Expected Proxy, got {:?}", other),
        }
    }

    // ── resolve_upstream: via upstream_id reference ───────────────

    #[test]
    fn handle_request_resolves_upstream_by_id() {
        let route: Route = serde_json::from_value(serde_json::json!({
            "id": "r1", "uri": "/ref-ups", "status": 1,
            "upstream_id": "ups1"
        })).unwrap();
        let cache = ConfigCache::new();
        let ups: Upstream = serde_json::from_value(serde_json::json!({
            "id": "ups1",
            "nodes": { "10.0.0.2:9090": 1 },
            "type": "roundrobin"
        })).unwrap();
        cache.upstreams.insert("ups1".to_string(), ups);

        let mut w = make_worker_with_registry(vec![route], PluginRegistry::new(), cache);
        let result = w.handle_request("GET", "/ref-ups", None, &[], "x");
        match result {
            RequestResult::Proxy { upstream_addr } => {
                assert_eq!(upstream_addr, "10.0.0.2:9090");
            }
            other => panic!("Expected Proxy, got {:?}", other),
        }
    }

    // ── resolve_upstream: via service_id → upstream ──────────────

    #[test]
    fn handle_request_resolves_upstream_via_service() {
        let route: Route = serde_json::from_value(serde_json::json!({
            "id": "r1", "uri": "/svc", "status": 1,
            "service_id": "svc1"
        })).unwrap();
        let cache = ConfigCache::new();
        let svc = ando_core::service::Service {
            id: "svc1".into(),
            name: None, desc: None,
            upstream_id: None,
            upstream: Some(serde_json::from_value(serde_json::json!({
                "nodes": { "10.0.0.3:7070": 1 },
                "type": "roundrobin"
            })).unwrap()),
            plugins: HashMap::new(),
            labels: HashMap::new(),
        };
        cache.services.insert("svc1".to_string(), svc);

        let mut w = make_worker_with_registry(vec![route], PluginRegistry::new(), cache);
        let result = w.handle_request("GET", "/svc", None, &[], "x");
        match result {
            RequestResult::Proxy { upstream_addr } => {
                assert_eq!(upstream_addr, "10.0.0.3:7070");
            }
            other => panic!("Expected Proxy, got {:?}", other),
        }
    }

    // ── pipeline cache: same route builds pipeline once ───────────

    #[test]
    fn pipeline_is_cached_across_requests() {
        let mut registry = PluginRegistry::new();
        ando_plugins::register_all(&mut registry);
        let route = route_with_key_auth("r1", "/cached", "127.0.0.1:8080");
        let mut w = make_worker_with_registry(vec![route], registry, ConfigCache::new());

        // First request — builds pipeline
        let _ = w.handle_request("GET", "/cached", None, &[("apikey", "k")], "x");
        assert!(w.pipeline_cache.contains_key("r1"), "pipeline must be cached");

        // Second request — uses cache (same route_id)
        let before_len = w.pipeline_cache.len();
        let _ = w.handle_request("GET", "/cached", None, &[("apikey", "k")], "x");
        assert_eq!(w.pipeline_cache.len(), before_len, "cache should not grow for same route");
    }

    // ── maybe_update_router clears pipeline cache ────────────────

    #[test]
    fn maybe_update_router_clears_pipeline_cache() {
        let mut registry = PluginRegistry::new();
        ando_plugins::register_all(&mut registry);
        let route = route_with_key_auth("r1", "/cached", "127.0.0.1:8080");
        let mut w = make_worker_with_registry(vec![route.clone()], registry, ConfigCache::new());

        // Prime the cache
        let _ = w.handle_request("GET", "/cached", None, &[("apikey", "k")], "x");
        assert!(!w.pipeline_cache.is_empty());

        // Config update (new version)
        let new_router = Arc::new(Router::build(vec![route], w.router_version + 1).unwrap());
        w.maybe_update_router(new_router);
        assert!(w.pipeline_cache.is_empty(), "pipeline cache must be cleared on router update");
    }

    // ── ConnPool: take from empty returns None ───────────────────

    #[test]
    fn conn_pool_take_empty_returns_none() {
        let mut pool = ConnPool::new(10);
        assert!(pool.take("127.0.0.1:8080").is_none());
    }

    // ── ConnPool: max_idle enforced ──────────────────────────────

    // NOTE: Cannot test put/take with real TcpStream in unit tests
    // (requires monoio runtime). ConnPool correctness is verified in
    // connection_integration.rs E2E tests.

    // ── build_upstream_request: no body = no content-length ──────

    #[test]
    fn build_upstream_request_no_body_no_content_length() {
        let mut buf = Vec::new();
        build_upstream_request(&mut buf, "GET", "/test", &[], b"");
        let text = String::from_utf8(buf).unwrap();
        assert!(!text.contains("content-length:"),
            "GET with empty body should not add content-length");
    }

    // ── build_response: non-standard status code ─────────────────

    #[test]
    fn build_response_non_standard_status_code() {
        let mut buf = Vec::new();
        build_response(&mut buf, 418, &[], b"I'm a teapot");
        let text = String::from_utf8(buf).unwrap();
        assert!(text.starts_with("HTTP/1.1 418 Unknown\r\n"));
        assert!(text.ends_with("I'm a teapot"));
    }

    // ── RESP_502 is valid HTTP ───────────────────────────────────

    #[test]
    fn resp_502_is_valid_http_response() {
        let text = String::from_utf8_lossy(RESP_502);
        assert!(text.starts_with("HTTP/1.1 502 Bad Gateway\r\n"));
        assert!(text.contains("content-type: application/json"));
        assert!(text.contains("upstream error"));
    }
}
