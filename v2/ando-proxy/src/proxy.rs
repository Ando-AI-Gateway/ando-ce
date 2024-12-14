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
        if pipeline.has_auth_plugins() {
            if let Some(key) = ctx.vars.get("_key_auth_key").and_then(|v| v.as_str()) {
                match self.consumer_keys.get(key) {
                    Some(username) => ctx.consumer = Some(username.clone()),
                    None => return RequestResult::Static(RESP_401_INVALID),
                }
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
        if let Some(ref ups) = route.upstream {
            if let Some(addr) = ups.first_node() {
                return addr.to_string();
            }
        }
        if let Some(ref id) = route.upstream_id {
            if let Some(ups) = self.upstreams.get(id) {
                if let Some(addr) = ups.first_node() {
                    return addr.to_string();
                }
            }
        }
        if let Some(ref svc_id) = route.service_id {
            if let Some(svc) = self.services.get(svc_id) {
                if let Some(ref ups) = svc.upstream {
                    if let Some(addr) = ups.first_node() {
                        return addr.to_string();
                    }
                }
                if let Some(ref ups_id) = svc.upstream_id {
                    if let Some(ups) = self.upstreams.get(ups_id) {
                        if let Some(addr) = ups.first_node() {
                            return addr.to_string();
                        }
                    }
                }
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
            if let Some(ref svc_id) = route.service_id {
                if let Some(svc) = self.services.get(svc_id) {
                    for (name, config) in &svc.plugins {
                        merged.insert(name.clone(), config.clone());
                    }
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
            if let Some(factory) = self.plugin_registry.get(name) {
                if let Ok(inst) = factory.configure(config) {
                    instances.push(Arc::from(inst));
                }
            }
        }

        let pipeline = Arc::new(PluginPipeline::build(instances, has_auth));
        self.pipeline_cache.insert(route_id.to_string(), Arc::clone(&pipeline));
        pipeline
    }
}

// ── Request result ────────────────────────────────────────────

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
/// Avoids TCP handshake on every request (saves ~1ms RTT).
pub struct ConnPool {
    pools: HashMap<String, VecDeque<TcpStream>>,
    max_idle: usize,
}

impl ConnPool {
    pub fn new(max_idle_per_host: usize) -> Self {
        Self {
            pools: HashMap::new(),
            max_idle: max_idle_per_host,
        }
    }

    pub fn take(&mut self, addr: &str) -> Option<TcpStream> {
        self.pools.get_mut(addr).and_then(|q| q.pop_front())
    }

    pub fn put(&mut self, addr: String, stream: TcpStream) {
        let queue = self.pools.entry(addr).or_insert_with(VecDeque::new);
        if queue.len() < self.max_idle {
            queue.push_back(stream);
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
