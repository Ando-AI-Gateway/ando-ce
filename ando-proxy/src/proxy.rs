use ando_core::router::Router;
use ando_observability::MetricsCollector;
use ando_plugin::plugin::{Phase, PluginContext, PluginResult};
use ando_plugin::registry::PluginRegistry;
use ando_plugin::pipeline::PluginPipeline;
use ando_plugin::plugin::PluginInstance;
use ando_store::cache::ConfigCache;
use async_trait::async_trait;
use pingora_core::prelude::*;
use pingora_http::ResponseHeader;
use pingora_proxy::{ProxyHttp, Session};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// The main Ando proxy service implementing Pingora's ProxyHttp trait.
pub struct AndoProxy {
    pub router: Arc<Router>,
    pub cache: ConfigCache,
    pub plugin_registry: Arc<PluginRegistry>,
    pub metrics: Arc<MetricsCollector>,
    pub logs_exporter: Arc<ando_observability::VictoriaLogsExporter>,
}

/// Per-request context stored in the Pingora session.
pub struct AndoCtx {
    pub route_id: String,
    pub plugin_ctx: PluginContext,
    pub upstream_addr: String,
    pub pipeline: Option<PluginPipeline>,
    pub request_start: std::time::Instant,
}

#[async_trait]
impl ProxyHttp for AndoProxy {
    type CTX = Option<AndoCtx>;

    fn new_ctx(&self) -> Self::CTX {
        None
    }

    /// Phase 1: Request filtering — route matching and pre-proxy plugins.
    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<bool> {
        let req = session.req_header();
        let method = req.method.as_str().to_string();
        let uri = req.uri.to_string();
        let host = req
            .headers
            .get("host")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Extract headers
        let mut headers = HashMap::new();
        for (name, value) in req.headers.iter() {
            headers.insert(
                name.as_str().to_lowercase(),
                value.to_str().unwrap_or("").to_string(),
            );
        }

        // Get client IP
        let client_ip = session
            .client_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        self.metrics.active_connections.inc();

        // Route matching
        let route_match = match self.router.match_route(
            &method,
            uri.split('?').next().unwrap_or(&uri),
            host.as_deref(),
        ) {
            Some(m) => m,
            None => {
                let mut resp = ResponseHeader::build(404, None)?;
                resp.insert_header("content-type", "application/json")?;
                session
                    .write_response_header(Box::new(resp), false)
                    .await?;
                session
                    .write_response_body(
                        Some(bytes::Bytes::from(
                            r#"{"error":"no route matched","status":404}"#,
                        )),
                        true,
                    )
                    .await?;
                self.metrics.active_connections.dec();
                return Ok(true);
            }
        };

        // Get route config
        let route = match self.router.get_route(&route_match.route_id) {
            Some(r) => r,
            None => {
                error!(route_id = %route_match.route_id, "Route found in match but not in store");
                self.metrics.active_connections.dec();
                return Ok(true);
            }
        };

        // Build plugin context
        let mut plugin_ctx = PluginContext::new(
            method.clone(),
            uri.clone(),
            headers,
            client_ip,
            route_match.route_id.clone(),
        );

        // Add path params
        for (k, v) in route_match.params {
            plugin_ctx.path_params.insert(k, v);
        }

        plugin_ctx.service_id = route.service_id.clone();

        // Merge plugins from various sources
        let mut merged_plugins = HashMap::new();

        if let Some(ref pc_id) = route.plugin_config_id {
            if let Some(pc) = self.cache.plugin_configs.get(pc_id) {
                for (name, config) in &pc.plugins {
                    merged_plugins.insert(name.clone(), config.clone());
                }
            }
        }

        if let Some(ref svc_id) = route.service_id {
            if let Some(svc) = self.cache.services.get(svc_id) {
                for (name, config) in &svc.plugins {
                    merged_plugins.insert(name.clone(), config.clone());
                }
            }
        }

        for (name, config) in &route.plugins {
            merged_plugins.insert(name.clone(), config.clone());
        }

        // Build plugin instances
        let mut instances = Vec::new();
        for (name, config) in &merged_plugins {
            if let Some(plugin) = self.plugin_registry.get(name) {
                instances.push(PluginInstance::new(plugin, config.clone()));
            } else {
                warn!(plugin = %name, "Plugin not found in registry, skipping");
            }
        }

        let pipeline = PluginPipeline::new(instances);

        // Execute pre-proxy phases
        match pipeline.execute_request_phases(&mut plugin_ctx).await {
            PluginResult::Continue => {}
            PluginResult::Response {
                status,
                headers: resp_headers,
                body,
            } => {
                let mut resp = ResponseHeader::build(status, None)?;
                resp.insert_header("content-type", "application/json")?;
                // Clone headers into owned strings for header insertion
                for (k, v) in resp_headers.iter() {
                    let k_owned = k.to_string();
                    let v_owned = v.to_string();
                    resp.insert_header(k_owned, v_owned)?;
                }
                session
                    .write_response_header(Box::new(resp), false)
                    .await?;
                if let Some(body) = body {
                    session
                        .write_response_body(Some(bytes::Bytes::from(body)), true)
                        .await?;
                }

                let elapsed = plugin_ctx.elapsed_ms();
                self.metrics
                    .record_request(&route_match.route_id, &method, status, elapsed / 1000.0);
                self.logs_exporter.access_log(
                    &route_match.route_id,
                    &method,
                    &uri,
                    status,
                    elapsed,
                    &plugin_ctx.client_ip,
                    None,
                );
                self.metrics.active_connections.dec();
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
                        Some(bytes::Bytes::from(
                            r#"{"error":"internal plugin error","status":500}"#,
                        )),
                        true,
                    )
                    .await?;
                self.metrics.active_connections.dec();
                return Ok(true);
            }
        }

        // Determine upstream address
        let upstream_addr = self.resolve_upstream(&route, &plugin_ctx);

        *ctx = Some(AndoCtx {
            route_id: route_match.route_id,
            plugin_ctx,
            upstream_addr: upstream_addr.unwrap_or_else(|| "127.0.0.1:80".to_string()),
            pipeline: Some(pipeline),
            request_start: std::time::Instant::now(),
        });

        Ok(false)
    }

    /// Phase 2: Select the upstream peer.
    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let ando_ctx = ctx.as_ref().expect("Context must be set by request_filter");

        let addr = &ando_ctx.upstream_addr;
        debug!(upstream = %addr, route = %ando_ctx.route_id, "Connecting to upstream");

        let peer = HttpPeer::new(addr.as_str(), false, String::new());
        Ok(Box::new(peer))
    }

    /// Phase 3: Modify response headers.
    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        // Add Ando gateway headers
        upstream_response.insert_header("x-powered-by", "Ando")?;

        let route_id = ctx.as_ref().map(|c| c.route_id.clone()).unwrap_or_default();
        upstream_response.insert_header("x-ando-route", route_id)?;

        if let Some(ando_ctx) = ctx {
            ando_ctx.plugin_ctx.response_status =
                Some(upstream_response.status.as_u16());

            // Execute header_filter phase
            if let Some(ref pipeline) = ando_ctx.pipeline {
                let _ = pipeline
                    .execute_phase(Phase::HeaderFilter, &mut ando_ctx.plugin_ctx)
                    .await;
            }

            // Apply any response headers set by plugins
            // Clone headers to avoid lifetime issues with insert_header
            let response_headers: Vec<(String, String)> = ando_ctx
                .plugin_ctx
                .response_headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            for (k, v) in response_headers {
                upstream_response.insert_header(k, v)?;
            }
        }

        Ok(())
    }

    /// Phase 4: Logging after response is sent.
    async fn logging(
        &self,
        _session: &mut Session,
        _error: Option<&pingora_core::Error>,
        ctx: &mut Self::CTX,
    ) {
        if let Some(ando_ctx) = ctx {
            let elapsed = ando_ctx.plugin_ctx.elapsed_ms();
            let status = ando_ctx
                .plugin_ctx
                .response_status
                .unwrap_or(0);

            // Execute log phase
            if let Some(ref pipeline) = ando_ctx.pipeline {
                pipeline.execute_log_phase(&mut ando_ctx.plugin_ctx).await;
            }

            // Record metrics
            self.metrics.record_request(
                &ando_ctx.route_id,
                &ando_ctx.plugin_ctx.request_method,
                status,
                elapsed / 1000.0,
            );

            // Send to VictoriaLogs
            self.logs_exporter.access_log(
                &ando_ctx.route_id,
                &ando_ctx.plugin_ctx.request_method,
                &ando_ctx.plugin_ctx.request_uri,
                status,
                elapsed,
                &ando_ctx.plugin_ctx.client_ip,
                Some(&ando_ctx.upstream_addr),
            );

            self.metrics.active_connections.dec();
        }
    }
}

impl AndoProxy {
    /// Resolve the upstream address for a route.
    fn resolve_upstream(
        &self,
        route: &ando_core::route::Route,
        ctx: &PluginContext,
    ) -> Option<String> {
        if let Some(ref addr) = ctx.upstream_addr {
            return Some(addr.clone());
        }

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
}
