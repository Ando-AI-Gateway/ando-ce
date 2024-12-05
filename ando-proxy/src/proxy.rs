use ando_core::router::Router;
use ando_observability::MetricsCollector;
use ando_plugin::plugin::{Phase, PluginContext, PluginResult};
use ando_plugin::registry::PluginRegistry;
use ando_plugin::pipeline::PluginPipeline;
use ando_plugin::plugin::PluginInstance;
use ando_store::cache::ConfigCache;
use async_trait::async_trait;
use pingora_core::prelude::*;
use pingora_core::upstreams::peer::PeerOptions;
use pingora_http::ResponseHeader;
use pingora_proxy::{ProxyHttp, Session};
use prometheus::Histogram;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::{error, warn};

// ── Thread-local fast-path cache ───────────────────────────────
// Since `work_stealing = false`, each connection stays on the same
// worker thread for its entire lifetime.  This lets us use a plain
// unsynchronised HashMap (no DashMap, no atomic locks, no shard
// hashing) for the most-accessed route data.
thread_local! {
    static FAST_CACHE: RefCell<ThreadLocalFastPath> = RefCell::new(ThreadLocalFastPath {
        version: 0,
        entries: HashMap::new(),
    });
}

struct ThreadLocalFastPath {
    version: u64,
    entries: HashMap<Arc<str>, FastPathEntry>,
}

struct FastPathEntry {
    has_plugins: bool,
    upstream_addr: Arc<str>,
    peer: HttpPeer,
}

// ── Static response bodies (avoid per-request allocation) ──────────
static NOT_FOUND_BODY: &[u8] = br#"{"error":"no route matched","status":404}"#;
static PLUGIN_ERR_BODY: &[u8] = br#"{"error":"internal plugin error","status":500}"#;

/// The main Ando proxy service implementing Pingora's ProxyHttp trait.
pub struct AndoProxy {
    pub router: Arc<Router>,
    pub cache: ConfigCache,
    pub plugin_registry: Arc<PluginRegistry>,
    pub metrics: Arc<MetricsCollector>,
    pub logs_exporter: Arc<ando_observability::VictoriaLogsExporter>,
    /// Pre-built, route-keyed plugin pipelines. Rebuilt only when routes change.
    pipeline_cache: dashmap::DashMap<Arc<str>, Arc<PluginPipeline>>,
    /// Mirrors Router::version(). When it drifts, the pipeline cache is flushed.
    pipeline_version: AtomicU64,
    /// Pre-resolved upstream addresses per route. Invalidated with pipeline cache.
    upstream_cache: dashmap::DashMap<Arc<str>, Arc<str>>,
    /// Pre-built HttpPeer per upstream address — avoids new() + address parsing per request.
    peer_cache: dashmap::DashMap<Arc<str>, HttpPeer>,
    /// Pre-computed route metadata (has_plugins, upstream_addr) — avoids cloning
    /// the full Route on every request on the fast path.
    route_meta_cache: dashmap::DashMap<Arc<str>, Arc<RouteMetadata>>,
    /// Pre-resolved per-route histogram handles — avoids with_label_values()
    /// HashMap lookup on every request.
    histogram_cache: dashmap::DashMap<Arc<str>, Histogram>,
    /// Pre-built peer options — avoids reconstructing timeouts on every request.
    peer_options: PeerOptions,
}

impl AndoProxy {
    pub fn new(
        router: Arc<Router>,
        cache: ConfigCache,
        plugin_registry: Arc<PluginRegistry>,
        metrics: Arc<MetricsCollector>,
        logs_exporter: Arc<ando_observability::VictoriaLogsExporter>,
    ) -> Self {
        // Pre-build peer options to avoid reconstructing timeouts on every request.
        let mut peer_options = PeerOptions::new();
        peer_options.connection_timeout = Some(Duration::from_secs(3));
        peer_options.read_timeout = Some(Duration::from_secs(15));
        peer_options.write_timeout = Some(Duration::from_secs(15));
        peer_options.idle_timeout = Some(Duration::from_secs(60));
        peer_options.total_connection_timeout = Some(Duration::from_secs(10));

        // NOTE: tcp_fast_open + tcp_keepalive intentionally disabled.
        // TFO: causes severe p99 spikes when backend doesn't support TFO.
        // tcp_keepalive: adds per-connection setsockopt overhead in benchmarks.

        Self {
            router,
            cache,
            plugin_registry,
            metrics,
            logs_exporter,
            pipeline_cache: dashmap::DashMap::new(),
            pipeline_version: AtomicU64::new(0),
            upstream_cache: dashmap::DashMap::new(),
            peer_cache: dashmap::DashMap::new(),
            route_meta_cache: dashmap::DashMap::new(),
            histogram_cache: dashmap::DashMap::new(),
            peer_options,
        }
    }

    /// Invalidate caches when the router version changes.
    #[inline(always)]
    fn maybe_invalidate_caches(&self) {
        let router_ver = self.router.version();
        let cached_ver = self.pipeline_version.load(Ordering::Relaxed);
        if cached_ver != router_ver {
            if self
                .pipeline_version
                .compare_exchange(cached_ver, router_ver, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                self.pipeline_cache.clear();
                self.upstream_cache.clear();
                self.peer_cache.clear();
                self.route_meta_cache.clear();
                self.histogram_cache.clear();
            }
        }
    }

    /// Resolve the upstream address for a route and cache it.
    /// Returns Arc<str> to avoid per-request String allocation.
    #[inline]
    fn resolve_upstream_cached(&self, route_id: &Arc<str>, route: &ando_core::route::Route) -> Arc<str> {
        if let Some(cached) = self.upstream_cache.get(route_id.as_ref()) {
            return Arc::clone(cached.value());
        }
        let addr = self
            .resolve_upstream_inner(route)
            .unwrap_or_else(|| "127.0.0.1:80".into());
        let arc: Arc<str> = Arc::from(addr.as_str());
        self.upstream_cache.insert(Arc::clone(route_id), Arc::clone(&arc));
        arc
    }

    /// Resolve the upstream address for a route (no caching layer).
    fn resolve_upstream_inner(&self, route: &ando_core::route::Route) -> Option<String> {
        if let Some(ref upstream) = route.upstream {
            return upstream.nodes.keys().next().map(|s| s.to_string());
        }

        if let Some(ref upstream_id) = route.upstream_id {
            if let Some(upstream) = self.cache.upstreams.get(upstream_id) {
                return upstream.nodes.keys().next().map(|s| s.to_string());
            }
        }

        if let Some(ref service_id) = route.service_id {
            if let Some(service) = self.cache.services.get(service_id) {
                if let Some(ref upstream) = service.upstream {
                    return upstream.nodes.keys().next().map(|s| s.to_string());
                }
                if let Some(ref upstream_id) = service.upstream_id {
                    if let Some(upstream) = self.cache.upstreams.get(upstream_id) {
                        return upstream.nodes.keys().next().map(|s| s.to_string());
                    }
                }
            }
        }

        None
    }

    /// Check if a route has any plugins (from route + service + plugin_config).
    #[inline]
    fn route_has_plugins(&self, route: &ando_core::route::Route) -> bool {
        if !route.plugins.is_empty() {
            return true;
        }
        if let Some(ref pc_id) = route.plugin_config_id {
            if let Some(pc) = self.cache.plugin_configs.get(pc_id) {
                if !pc.plugins.is_empty() {
                    return true;
                }
            }
        }
        if let Some(ref svc_id) = route.service_id {
            if let Some(svc) = self.cache.services.get(svc_id) {
                if !svc.plugins.is_empty() {
                    return true;
                }
            }
        }
        false
    }

    /// Get or build route metadata. After first build per route, subsequent
    /// requests just do a DashMap lookup (no Route clone).
    #[inline]
    fn get_route_metadata(&self, route_id: &Arc<str>) -> Option<Arc<RouteMetadata>> {
        // Fast path: already cached
        if let Some(cached) = self.route_meta_cache.get(route_id.as_ref()) {
            return Some(Arc::clone(cached.value()));
        }
        // Slow path: clone Route once, compute metadata, cache it
        let route = self.router.get_route(route_id)?;
        let has_plugins = self.route_has_plugins(&route);
        let upstream_addr = self.resolve_upstream_cached(route_id, &route);
        let meta = Arc::new(RouteMetadata {
            has_plugins,
            upstream_addr,
        });
        self.route_meta_cache
            .insert(Arc::clone(route_id), Arc::clone(&meta));
        Some(meta)
    }

    /// Merge plugins from all sources for a route.
    fn merge_plugins(&self, route: &ando_core::route::Route) -> HashMap<String, serde_json::Value> {
        let mut merged = HashMap::new();

        if let Some(ref pc_id) = route.plugin_config_id {
            if let Some(pc) = self.cache.plugin_configs.get(pc_id) {
                for (name, config) in &pc.plugins {
                    merged.insert(name.clone(), config.clone());
                }
            }
        }

        if let Some(ref svc_id) = route.service_id {
            if let Some(svc) = self.cache.services.get(svc_id) {
                for (name, config) in &svc.plugins {
                    merged.insert(name.clone(), config.clone());
                }
            }
        }

        for (name, config) in &route.plugins {
            merged.insert(name.clone(), config.clone());
        }

        merged
    }

    /// Get or build the plugin pipeline for a route (cached).
    fn get_pipeline(
        &self,
        route_id: &Arc<str>,
        merged_plugins: &HashMap<String, serde_json::Value>,
    ) -> Arc<PluginPipeline> {
        if let Some(cached) = self.pipeline_cache.get(route_id.as_ref()) {
            return Arc::clone(cached.value());
        }
        let mut instances = Vec::with_capacity(merged_plugins.len());
        for (name, config) in merged_plugins {
            if let Some(plugin) = self.plugin_registry.get(name) {
                instances.push(PluginInstance::new(plugin, config.clone()));
            } else {
                warn!(plugin = %name, "Plugin not found in registry, skipping");
            }
        }
        let built = Arc::new(PluginPipeline::new(instances));
        self.pipeline_cache
            .insert(Arc::clone(route_id), Arc::clone(&built));
        built
    }

    /// Build or retrieve an HttpPeer for the given upstream address.
    #[inline]
    fn build_or_get_peer(&self, addr: &Arc<str>) -> HttpPeer {
        if let Some(cached) = self.peer_cache.get(addr) {
            return cached.value().clone();
        }
        let mut peer = HttpPeer::new(addr.as_ref(), false, String::new());
        peer.options = self.peer_options.clone();
        self.peer_cache.insert(Arc::clone(addr), peer.clone());
        peer
    }
}

/// Per-request context stored in the Pingora session.
///
/// **Size-optimised for cache locality**: large fields (`PluginContext`,
/// `HttpPeer`) are boxed so the inline struct fits in ~88 bytes (~1 cache
/// line) instead of ~850+ bytes.  At 200 concurrent connections across 8
/// workers every Tokio task future carries this struct, so shrinking it
/// dramatically reduces L1-D cache pressure.
pub struct AndoCtx {
    pub route_id: Arc<str>,
    /// Boxed to avoid ~500+ bytes inline when None on the fast path.
    pub plugin_ctx: Option<Box<PluginContext>>,
    pub upstream_addr: Arc<str>,
    pub pipeline: Option<Arc<PluginPipeline>>,
    /// Only meaningful on slow path (with plugins). Fast path skips setting
    /// this (uses Instant from UNIX_EPOCH as sentinel) and never reads it
    /// because logging() returns immediately for no-plugin routes.
    pub request_start: std::time::Instant,
    /// Pre-built, **already-boxed** peer from thread-local cache.
    /// `upstream_peer()` simply moves this box out — zero clone, zero alloc.
    cached_peer: Option<Box<HttpPeer>>,
    /// Tracks whether `fail_to_connect` has already retried once for this request.
    retried: bool,
}

/// Cheap `Instant` sentinel — avoids calling `Instant::now()` on the fast
/// path where `request_start` is never read.
#[inline(always)]
fn instant_sentinel() -> std::time::Instant {
    // Instant::now() involves a syscall/vDSO call. On the fast path (no
    // plugins) the value is never consumed so we avoid the call entirely.
    // Safety: we need a valid Instant. The cheapest way is to cache one.
    static SENTINEL: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *SENTINEL.get_or_init(std::time::Instant::now)
}

/// Cached route metadata — avoids cloning the full Route struct on every request.
/// Only holds the fields needed for the hot path decision: does this route have
/// plugins, and what is its upstream address?
struct RouteMetadata {
    has_plugins: bool,
    upstream_addr: Arc<str>,
}

#[async_trait]
impl ProxyHttp for AndoProxy {
    type CTX = Option<AndoCtx>;

    fn new_ctx(&self) -> Self::CTX {
        None
    }

    /// Override: skip default ResponseCompressionBuilder module.
    /// We don't use response compression; removing it eliminates per-request
    /// module dispatch overhead that Pingora's default adds even when disabled.
    fn init_downstream_modules(&self, _modules: &mut pingora_core::modules::http::HttpModules) {
        // intentionally empty — no downstream modules needed
    }

    /// Phase 1: Request filtering — route matching and pre-proxy plugins.
    ///
    /// HOT PATH — every micro-allocation here matters at >100K req/s.
    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<bool> {
        let req = session.req_header();
        let method = req.method.as_str();
        let uri_raw = req.uri.path();
        let host = req
            .headers
            .get("host")
            .and_then(|v| v.to_str().ok());

        // ── Route matching (zero-copy — borrows from req header) ──
        let route_match = match self.router.match_route(method, uri_raw, host) {
            Some(m) => m,
            None => {
                let mut resp = ResponseHeader::build(404, None)?;
                resp.insert_header("content-type", "application/json")?;
                session
                    .write_response_header(Box::new(resp), false)
                    .await?;
                session
                    .write_response_body(
                        Some(bytes::Bytes::from_static(NOT_FOUND_BODY)),
                        true,
                    )
                    .await?;
                return Ok(true);
            }
        };

        // ── Thread-local fast-path lookup ──────────────────────────────
        // One unsynchronised HashMap get replaces three DashMap gets
        // (route_meta_cache + upstream_cache + peer_cache).
        // NOTE: version check here also handles cache invalidation —
        // we skip maybe_invalidate_caches() on the ultra-fast path
        // (thread-local hit + no plugins) to avoid touching atomics/DashMap.
        let router_ver = self.router.version();
        let fast_hit = FAST_CACHE.with(|cell| {
            let mut fp = cell.borrow_mut();
            if fp.version != router_ver {
                fp.entries.clear();
                fp.version = router_ver;
            }
            fp.entries.get(&route_match.route_id).map(|e| {
                // Clone the template peer directly into a Box — the Box is
                // moved as-is into upstream_peer() (zero extra alloc there).
                (e.has_plugins, Arc::clone(&e.upstream_addr), Box::new(e.peer.clone()))
            })
        });

        let had_fast_hit = fast_hit.is_some();

        if let Some((has_plugins, upstream_addr, peer)) = fast_hit {
            if !has_plugins {
                // Ultra-fast path: thread-local hit + no plugins.
                // Zero synchronised data-structure access.
                // AndoCtx is ~88 bytes — fits in 1-2 cache lines.
                *ctx = Some(AndoCtx {
                    route_id: route_match.route_id,
                    plugin_ctx: None,
                    upstream_addr,
                    pipeline: None,
                    request_start: instant_sentinel(),
                    cached_peer: Some(peer),
                    retried: false,
                });
                return Ok(false);
            }
            // has_plugins = true → fall through to slow path below
        }

        // ── Slow path: invalidate DashMap caches if version changed ──
        // Only called on thread-local miss — avoids extra atomic ops on fast path.
        self.maybe_invalidate_caches();

        // ── Route metadata cache: has_plugins + upstream_addr ──
        // After the first request for a route, subsequent requests only do
        // a DashMap get (no Route clone at all).
        let meta = match self.get_route_metadata(&route_match.route_id) {
            Some(m) => m,
            None => {
                error!(route_id = %route_match.route_id, "Route found in match but not in store");
                return Ok(true);
            }
        };

        // Populate thread-local cache for next request on this thread
        if !had_fast_hit {
            let peer = self.build_or_get_peer(&meta.upstream_addr);
            FAST_CACHE.with(|cell| {
                let mut fp = cell.borrow_mut();
                fp.entries.insert(Arc::clone(&route_match.route_id), FastPathEntry {
                    has_plugins: meta.has_plugins,
                    upstream_addr: Arc::clone(&meta.upstream_addr),
                    peer,
                });
            });
        }

        // ──────────────────────────────────────────────────────────────
        // FAST PATH: no plugins → skip PluginContext + pipeline entirely
        // ──────────────────────────────────────────────────────────────
        if !meta.has_plugins {
            *ctx = Some(AndoCtx {
                route_id: route_match.route_id,
                plugin_ctx: None,
                upstream_addr: Arc::clone(&meta.upstream_addr),
                pipeline: None,
                request_start: instant_sentinel(),
                cached_peer: None, // will use peer_cache in upstream_peer
                retried: false,
            });
            return Ok(false);
        }

        // ──────────────────────────────────────────────────────────────
        // SLOW PATH: plugins present — full context creation
        // Need the full Route for plugin merging.
        // ──────────────────────────────────────────────────────────────
        let route = match self.router.get_route(&route_match.route_id) {
            Some(r) => r,
            None => return Ok(true),
        };
        let method_owned = method.to_string();
        let uri_string = req.uri.to_string();

        // Extract headers only when plugins need them
        let mut headers = HashMap::with_capacity(req.headers.len());
        for (name, value) in req.headers.iter() {
            headers.insert(
                name.as_str().to_lowercase(),
                value.to_str().unwrap_or("").to_string(),
            );
        }

        let client_ip = session
            .client_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".into());

        let mut plugin_ctx = PluginContext::new(
            method_owned.clone(),
            uri_string.clone(),
            headers,
            client_ip,
            route_match.route_id.to_string(),
        );

        // Add path params
        for (k, v) in route_match.params {
            plugin_ctx.path_params.insert(k, v);
        }

        plugin_ctx.service_id = route.service_id.clone();

        // Merge plugins from all sources
        let merged_plugins = self.merge_plugins(&route);

        // Inject consumer snapshot only when auth plugins are present
        let needs_consumers = merged_plugins.keys().any(|k| {
            matches!(
                k.as_str(),
                "key-auth" | "jwt-auth" | "basic-auth" | "hmac-auth" | "consumer-restriction"
            )
        });
        if needs_consumers {
            for entry in self.cache.consumers.iter() {
                plugin_ctx
                    .consumers
                    .insert(entry.key().clone(), entry.value().clone());
            }
        }

        let pipeline = self.get_pipeline(&route_match.route_id, &merged_plugins);

        // Execute pre-proxy phases
        match pipeline.execute_request_phases(&mut plugin_ctx).await {
            PluginResult::Continue => {}
            PluginResult::Response {
                status,
                headers: resp_headers,
                body,
            } => {
                let has_body = body.is_some();
                let mut resp = ResponseHeader::build(status, None)?;
                resp.insert_header("content-type", "application/json")?;
                for (k, v) in resp_headers {
                    resp.insert_header(k, v)?;
                }
                // Signal end_of_stream correctly:
                // - If body is present → header is not end, body is end
                // - If body is None   → header IS end (fixes connection hang)
                session
                    .write_response_header(Box::new(resp), !has_body)
                    .await?;
                if let Some(body) = body {
                    session
                        .write_response_body(Some(bytes::Bytes::from(body)), true)
                        .await?;
                }

                let elapsed = plugin_ctx.elapsed_ms();
                self.metrics
                    .record_request(&route_match.route_id, &method_owned, status, elapsed / 1000.0);
                return Ok(true);
            }
            PluginResult::Error(msg) => {
                error!(error = %msg, "Plugin pipeline error");
                let mut resp = ResponseHeader::build(500, None)?;
                resp.insert_header("content-type", "application/json")?;
                session
                    .write_response_header(Box::new(resp), false)
                    .await?;
                session
                    .write_response_body(
                        Some(bytes::Bytes::from_static(PLUGIN_ERR_BODY)),
                        true,
                    )
                    .await?;
                return Ok(true);
            }
        }

        // Check if plugin overrode the upstream
        let final_upstream = if let Some(ref addr) = plugin_ctx.upstream_addr {
            Arc::from(addr.as_str())
        } else {
            Arc::clone(&meta.upstream_addr)
        };

        *ctx = Some(AndoCtx {
            route_id: route_match.route_id,
            plugin_ctx: Some(Box::new(plugin_ctx)),
            upstream_addr: final_upstream,
            pipeline: Some(pipeline),
            request_start: std::time::Instant::now(),
            cached_peer: None,
            retried: false,
        });

        Ok(false)
    }

    /// Phase 2: Select the upstream peer.
    ///
    /// If the thread-local fast path populated cached_peer in request_filter,
    /// the Box is simply moved out — zero clone, zero allocation.
    /// Otherwise falls back to the DashMap peer_cache.
    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let ando_ctx = ctx.as_mut().expect("Context must be set by request_filter");

        // Ultra-fast path: pre-boxed peer from thread-local cache.
        // Just moves the Box — no clone, no allocation.
        if let Some(peer) = ando_ctx.cached_peer.take() {
            return Ok(peer);
        }

        // Slow path: DashMap lookup → clone + Box
        let addr = &ando_ctx.upstream_addr;
        if let Some(cached) = self.peer_cache.get(addr) {
            return Ok(Box::new(cached.value().clone()));
        }

        let mut peer = HttpPeer::new(addr.as_ref(), false, String::new());
        peer.options = self.peer_options.clone();
        self.peer_cache.insert(Arc::clone(addr), peer.clone());
        Ok(Box::new(peer))
    }

    /// Phase 3: Modify response headers — only run plugin phases.
    ///
    /// FAST PATH (no plugins): early return — zero work.
    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let ando_ctx = match ctx {
            Some(c) if c.pipeline.is_some() => c,
            _ => return Ok(()), // fast path: no plugins → nothing to do
        };

        ando_ctx.request_start = std::time::Instant::now(); // reset for response timing

        if let Some(plugin_ctx) = &mut ando_ctx.plugin_ctx {
            plugin_ctx.response_status = Some(upstream_response.status.as_u16());

            if let Some(pipeline) = &ando_ctx.pipeline {
                if pipeline.has_phase(Phase::HeaderFilter) {
                    let _ = pipeline
                        .execute_phase(Phase::HeaderFilter, plugin_ctx)
                        .await;

                    if !plugin_ctx.response_headers.is_empty() {
                        let hdrs: Vec<(String, String)> = plugin_ctx
                            .response_headers
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        for (k, v) in hdrs {
                            upstream_response.insert_header(k, v)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Phase 4: Logging after response is sent — fire-and-forget metrics.
    ///
    /// FAST PATH (no plugins): complete no-op.  This eliminates ALL
    /// prometheus with_label_values() HashMap lookups, atomic counter
    /// increments, and histogram observations from the hot path, saving
    /// ~200-500ns per request.
    async fn logging(
        &self,
        session: &mut Session,
        _error: Option<&pingora_core::Error>,
        ctx: &mut Self::CTX,
    ) {
        if let Some(ando_ctx) = ctx {
            // Fast path: no plugins → skip ALL metrics + logging
            if ando_ctx.pipeline.is_none() {
                return;
            }

            let status = ando_ctx
                .plugin_ctx
                .as_ref()
                .and_then(|pc| pc.response_status)
                .unwrap_or(200);

            // Execute log phase only if plugins present
            if let Some(pipeline) = &ando_ctx.pipeline {
                if pipeline.has_phase(Phase::Log) {
                    if let Some(plugin_ctx) = &mut ando_ctx.plugin_ctx {
                        pipeline.execute_log_phase(plugin_ctx).await;
                    }
                }
            }

            // Record metrics only for the slow path (with plugins)
            let req = session.req_header();
            let method = req.method.as_str();
            let uri = req.uri.path();
            let elapsed_secs = ando_ctx.request_start.elapsed().as_secs_f64();

            {
                let mut buf = itoa::Buffer::new();
                let status_str = buf.format(status);
                self.metrics
                    .http_requests_total
                    .with_label_values(&[&ando_ctx.route_id, method, status_str])
                    .inc();
            }

            if let Some(h) = self.histogram_cache.get(ando_ctx.route_id.as_ref()) {
                h.value().observe(elapsed_secs);
            } else {
                let h = self
                    .metrics
                    .http_request_duration
                    .with_label_values(&[&ando_ctx.route_id]);
                h.observe(elapsed_secs);
                self.histogram_cache
                    .insert(Arc::clone(&ando_ctx.route_id), h);
            }

            let client_ip = ando_ctx
                .plugin_ctx
                .as_ref()
                .map(|pc| pc.client_ip.as_str())
                .unwrap_or("unknown");
            self.logs_exporter.access_log(
                &ando_ctx.route_id,
                method,
                uri,
                status,
                elapsed_secs * 1000.0,
                client_ip,
                Some(&ando_ctx.upstream_addr),
            );
        }
    }

    // ── Error handling — Pingora best practices ──────────────────────

    /// Suppress noisy downstream client disconnect errors.
    ///
    /// These occur when clients close connections with in-flight requests
    /// (e.g., during benchmark shutdown or client timeouts) and are
    /// expected in production — logging them just wastes I/O.
    fn suppress_error_log(
        &self,
        _session: &Session,
        _ctx: &Self::CTX,
        error: &pingora_core::Error,
    ) -> bool {
        error.source_str() == "Downstream"
    }

    /// Retry on stale upstream pool connections (idempotent methods only).
    ///
    /// When a keepalive connection from the pool has been closed by the
    /// upstream (stale), Pingora calls this with `client_reused = true`.
    /// For safe/idempotent HTTP methods we mark the error as retryable
    /// so Pingora transparently reconnects instead of returning 502.
    fn error_while_proxy(
        &self,
        _peer: &HttpPeer,
        session: &mut Session,
        mut e: Box<pingora_core::Error>,
        _ctx: &mut Self::CTX,
        client_reused: bool,
    ) -> Box<pingora_core::Error> {
        if client_reused {
            let method = session.req_header().method.as_str();
            if matches!(method, "GET" | "HEAD" | "OPTIONS" | "PUT" | "DELETE") {
                e.set_retry(true);
            }
        }
        e
    }

    /// Allow one retry on upstream connection failure.
    ///
    /// At this point nothing has been sent upstream, so retrying is always
    /// safe regardless of HTTP method. We limit to one retry to avoid
    /// infinite loops on truly unreachable backends.
    fn fail_to_connect(
        &self,
        _session: &mut Session,
        _peer: &HttpPeer,
        ctx: &mut Self::CTX,
        mut e: Box<pingora_core::Error>,
    ) -> Box<pingora_core::Error> {
        if let Some(ando_ctx) = ctx.as_mut() {
            if !ando_ctx.retried {
                ando_ctx.retried = true;
                e.set_retry(true);
            }
        }
        e
    }
}
