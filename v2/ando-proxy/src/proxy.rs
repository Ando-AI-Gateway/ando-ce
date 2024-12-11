use ando_core::config::GatewayConfig;
use ando_core::router::Router;
use ando_plugin::pipeline::PluginPipeline;
use ando_plugin::plugin::{Phase, PluginContext, PluginInstance, PluginResult};
use ando_plugin::registry::PluginRegistry;
use ando_store::cache::ConfigCache;
use std::collections::HashMap;
use std::sync::Arc;

/// Per-worker-core proxy state. Completely owned — no shared mutable state.
///
/// v2 Architecture:
/// ────────────────
/// Each monoio worker thread owns an independent ProxyWorker.
/// All data structures are thread-local:
///   - Router: Arc<Router> swapped atomically via arc_swap
///   - Pipeline cache: plain HashMap (no DashMap)
///   - Upstream connections: local pool per core
///   - Metrics: local counters flushed periodically
///
/// Config updates arrive via a crossbeam SPSC channel. On receive,
/// the worker rebuilds its local caches from the new Arc<Router>.
///
/// This design eliminates:
///   - All atomic operations on the hot path
///   - All DashMap shard locking
///   - All cross-core cache line bouncing
///   - All Tokio runtime overhead (task scheduling, future polling)
pub struct ProxyWorker {
    /// Thread-local router snapshot.
    router: Arc<Router>,
    /// Router version for invalidation.
    router_version: u64,
    /// Thread-local pipeline cache.
    pipeline_cache: HashMap<String, Arc<PluginPipeline>>,
    /// Thread-local upstream address cache.
    upstream_cache: HashMap<String, String>,
    /// Thread-local route metadata.
    route_meta: HashMap<String, RouteMeta>,
    /// Plugin registry (shared, immutable -- read-only after startup).
    plugin_registry: Arc<PluginRegistry>,
    /// Config cache (shared, read-only snapshot access).
    config_cache: ConfigCache,
    /// Consumer key index (local copy for O(1) auth lookup).
    consumer_keys: HashMap<String, String>,
    /// Gateway config.
    _config: Arc<GatewayConfig>,
}

/// Minimal per-route metadata cached locally per worker.
struct RouteMeta {
    has_plugins: bool,
    upstream_addr: String,
}

impl ProxyWorker {
    pub fn new(
        router: Arc<Router>,
        plugin_registry: Arc<PluginRegistry>,
        config_cache: ConfigCache,
        config: Arc<GatewayConfig>,
    ) -> Self {
        let mut consumer_keys = HashMap::new();
        for entry in config_cache.consumer_key_index.iter() {
            consumer_keys.insert(entry.key().clone(), entry.value().clone());
        }

        Self {
            router_version: router.version(),
            router,
            pipeline_cache: HashMap::new(),
            upstream_cache: HashMap::new(),
            route_meta: HashMap::new(),
            plugin_registry,
            config_cache,
            consumer_keys,
            _config: config,
        }
    }

    /// Check for config updates from the control plane.
    pub fn maybe_update_router(&mut self, new_router: Arc<Router>) {
        let new_ver = new_router.version();
        if new_ver != self.router_version {
            self.router = new_router;
            self.router_version = new_ver;
            self.pipeline_cache.clear();
            self.upstream_cache.clear();
            self.route_meta.clear();
            // Rebuild consumer key index
            self.consumer_keys.clear();
            for entry in self.config_cache.consumer_key_index.iter() {
                self.consumer_keys.insert(entry.key().clone(), entry.value().clone());
            }
        }
    }

    /// Process an HTTP request. Returns (status, headers, body).
    ///
    /// This is the hot path — every allocation here costs throughput.
    pub fn handle_request(
        &mut self,
        method: &str,
        path: &str,
        host: Option<&str>,
        headers: &[(String, String)],
        client_ip: &str,
    ) -> HttpResponse {
        // ── Route matching ──
        let route_match = match self.router.match_route(method, path, host) {
            Some(m) => m,
            None => {
                return HttpResponse {
                    status: 404,
                    headers: vec![("content-type".into(), "application/json".into())],
                    body: br#"{"error":"no route matched","status":404}"#.to_vec(),
                    upstream_addr: None,
                };
            }
        };

        let route_id = &route_match.route_id;

        // ── Route metadata (local cache) ──
        let meta = self.get_or_build_meta(route_id);
        let has_plugins = meta.has_plugins;
        let upstream_addr = meta.upstream_addr.clone();

        // ── FAST PATH: no plugins → forward directly ──
        if !has_plugins {
            return HttpResponse {
                status: 0, // 0 = proxy to upstream
                headers: vec![],
                body: vec![],
                upstream_addr: Some(upstream_addr),
            };
        }

        // ── SLOW PATH: execute plugin pipeline ──
        let pipeline = self.get_or_build_pipeline(route_id);

        let header_map: HashMap<String, String> = headers
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.clone()))
            .collect();

        let mut ctx = PluginContext::new(
            route_id.clone(),
            client_ip.to_string(),
            method.to_string(),
            path.to_string(),
            header_map,
        );

        // Execute rewrite + access phases
        for phase in &[Phase::Rewrite, Phase::Access] {
            match pipeline.execute_phase(*phase, &mut ctx) {
                PluginResult::Continue => {}
                PluginResult::Response { status, headers, body } => {
                    return HttpResponse {
                        status,
                        headers,
                        body: body.unwrap_or_default(),
                        upstream_addr: None,
                    };
                }
            }
        }

        // ── Consumer validation (for auth plugins) ──
        if pipeline.has_auth_plugins() {
            if let Some(key_value) = ctx.vars.get("_key_auth_key").and_then(|v| v.as_str()) {
                match self.consumer_keys.get(key_value) {
                    Some(username) => {
                        ctx.consumer = Some(username.clone());
                    }
                    None => {
                        return HttpResponse {
                            status: 401,
                            headers: vec![("content-type".into(), "application/json".into())],
                            body: br#"{"error":"Invalid API key","status":401}"#.to_vec(),
                            upstream_addr: None,
                        };
                    }
                }
            }
        }

        // Execute before_proxy phase
        match pipeline.execute_phase(Phase::BeforeProxy, &mut ctx) {
            PluginResult::Continue => {}
            PluginResult::Response { status, headers, body } => {
                return HttpResponse {
                    status,
                    headers,
                    body: body.unwrap_or_default(),
                    upstream_addr: None,
                };
            }
        }

        HttpResponse {
            status: 0, // proxy to upstream
            headers: ctx.response_headers.into_iter().collect(),
            body: vec![],
            upstream_addr: Some(upstream_addr),
        }
    }

    fn get_or_build_meta(&mut self, route_id: &str) -> &RouteMeta {
        if !self.route_meta.contains_key(route_id) {
            let route = self.router.get_route(route_id);
            let (has_plugins, upstream_addr) = if let Some(route) = route {
                let has_plugins = !route.plugins.is_empty()
                    || route.plugin_config_id.is_some()
                    || route.service_id.is_some();
                let addr = self.resolve_upstream(route_id, route);
                (has_plugins, addr)
            } else {
                (false, "127.0.0.1:80".to_string())
            };

            self.route_meta.insert(
                route_id.to_string(),
                RouteMeta {
                    has_plugins,
                    upstream_addr,
                },
            );
        }
        self.route_meta.get(route_id).unwrap()
    }

    fn resolve_upstream(&self, _route_id: &str, route: &ando_core::route::Route) -> String {
        // Direct upstream on route
        if let Some(ref upstream) = route.upstream {
            if let Some(addr) = upstream.first_node() {
                return addr.to_string();
            }
        }
        // Upstream by ID
        if let Some(ref upstream_id) = route.upstream_id {
            if let Some(entry) = self.config_cache.upstreams.get(upstream_id) {
                if let Some(addr) = entry.value().first_node() {
                    return addr.to_string();
                }
            }
        }
        // Service → upstream
        if let Some(ref service_id) = route.service_id {
            if let Some(svc) = self.config_cache.services.get(service_id) {
                if let Some(ref ups) = svc.value().upstream {
                    if let Some(addr) = ups.first_node() {
                        return addr.to_string();
                    }
                }
                if let Some(ref ups_id) = svc.value().upstream_id {
                    if let Some(ups) = self.config_cache.upstreams.get(ups_id) {
                        if let Some(addr) = ups.value().first_node() {
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
        let mut merged_plugins: HashMap<String, serde_json::Value> = HashMap::new();
        let mut has_auth = false;

        if let Some(route) = route {
            // Merge from plugin_config
            if let Some(ref pc_id) = route.plugin_config_id {
                if let Some(pc) = self.config_cache.plugin_configs.get(pc_id) {
                    for (name, config) in &pc.value().plugins {
                        merged_plugins.insert(name.clone(), config.clone());
                    }
                }
            }
            // Merge from service
            if let Some(ref svc_id) = route.service_id {
                if let Some(svc) = self.config_cache.services.get(svc_id) {
                    for (name, config) in &svc.value().plugins {
                        merged_plugins.insert(name.clone(), config.clone());
                    }
                }
            }
            // Route-level plugins override
            for (name, config) in &route.plugins {
                merged_plugins.insert(name.clone(), config.clone());
            }
        }

        // Build plugin instances
        let mut instances: Vec<Arc<dyn PluginInstance>> = Vec::new();
        for (name, config) in &merged_plugins {
            if name == "key-auth" || name == "jwt-auth" || name == "basic-auth" {
                has_auth = true;
            }
            if let Some(factory) = self.plugin_registry.get(name) {
                match factory.configure(config) {
                    Ok(instance) => instances.push(Arc::from(instance)),
                    Err(e) => {
                        tracing::error!(plugin = %name, error = %e, "Failed to configure plugin");
                    }
                }
            } else {
                tracing::warn!(plugin = %name, "Unknown plugin, skipping");
            }
        }

        let pipeline = Arc::new(PluginPipeline::build(instances, has_auth));
        self.pipeline_cache
            .insert(route_id.to_string(), Arc::clone(&pipeline));
        pipeline
    }
}

/// HTTP response from the proxy logic.
pub struct HttpResponse {
    /// HTTP status code. 0 means "proxy to upstream".
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Target upstream address (only set when status = 0).
    pub upstream_addr: Option<String>,
}
