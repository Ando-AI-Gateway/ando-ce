use ando_core::config::VictoriaMetricsConfig;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts, Registry, TextEncoder,
};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};

/// Metrics collector for the gateway.
///
/// Collects request metrics, latency histograms, and system gauges.
/// Supports both Prometheus pull (text exposition) and VictoriaMetrics push.
pub struct MetricsCollector {
    registry: Registry,

    /// Total HTTP requests by route, method, status
    pub http_requests_total: IntCounterVec,

    /// Request latency histogram by route
    pub http_request_duration: HistogramVec,

    /// Active connections gauge
    pub active_connections: IntGauge,

    /// Upstream response time histogram
    pub upstream_latency: HistogramVec,

    /// Bandwidth counters
    pub ingress_bytes: IntCounterVec,
    pub egress_bytes: IntCounterVec,

    /// Plugin execution time histogram
    pub plugin_execution_time: HistogramVec,

    /// Lua VM pool stats
    pub lua_pool_available: IntGauge,
    pub lua_pool_in_use: IntGauge,
}

impl MetricsCollector {
    pub fn new() -> anyhow::Result<Self> {
        let registry = Registry::new();

        let http_requests_total = IntCounterVec::new(
            Opts::new("ando_http_requests_total", "Total HTTP requests")
                .namespace("ando"),
            &["route", "method", "status"],
        )?;

        let http_request_duration = HistogramVec::new(
            HistogramOpts::new("ando_http_request_duration_seconds", "Request latency")
                .namespace("ando")
                .buckets(vec![
                    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
                ]),
            &["route"],
        )?;

        let active_connections = IntGauge::new(
            "ando_active_connections",
            "Number of active connections",
        )?;

        let upstream_latency = HistogramVec::new(
            HistogramOpts::new("ando_upstream_latency_seconds", "Upstream response time")
                .namespace("ando")
                .buckets(vec![
                    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
                ]),
            &["upstream"],
        )?;

        let ingress_bytes = IntCounterVec::new(
            Opts::new("ando_ingress_bytes_total", "Total ingress bandwidth")
                .namespace("ando"),
            &["route"],
        )?;

        let egress_bytes = IntCounterVec::new(
            Opts::new("ando_egress_bytes_total", "Total egress bandwidth")
                .namespace("ando"),
            &["route"],
        )?;

        let plugin_execution_time = HistogramVec::new(
            HistogramOpts::new(
                "ando_plugin_execution_seconds",
                "Plugin execution time",
            )
            .namespace("ando")
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
            &["plugin", "phase"],
        )?;

        let lua_pool_available = IntGauge::new(
            "ando_lua_pool_available",
            "Available Lua VMs in pool",
        )?;

        let lua_pool_in_use = IntGauge::new(
            "ando_lua_pool_in_use",
            "In-use Lua VMs",
        )?;

        // Register all metrics
        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(http_request_duration.clone()))?;
        registry.register(Box::new(active_connections.clone()))?;
        registry.register(Box::new(upstream_latency.clone()))?;
        registry.register(Box::new(ingress_bytes.clone()))?;
        registry.register(Box::new(egress_bytes.clone()))?;
        registry.register(Box::new(plugin_execution_time.clone()))?;
        registry.register(Box::new(lua_pool_available.clone()))?;
        registry.register(Box::new(lua_pool_in_use.clone()))?;

        Ok(Self {
            registry,
            http_requests_total,
            http_request_duration,
            active_connections,
            upstream_latency,
            ingress_bytes,
            egress_bytes,
            plugin_execution_time,
            lua_pool_available,
            lua_pool_in_use,
        })
    }

    /// Record a completed HTTP request.
    pub fn record_request(
        &self,
        route: &str,
        method: &str,
        status: u16,
        duration_secs: f64,
    ) {
        self.http_requests_total
            .with_label_values(&[route, method, &status.to_string()])
            .inc();
        self.http_request_duration
            .with_label_values(&[route])
            .observe(duration_secs);
    }

    /// Get Prometheus text exposition.
    pub fn gather_text(&self) -> String {
        let encoder = TextEncoder::new();
        let metrics = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metrics, &mut buffer).unwrap_or_default();
        String::from_utf8(buffer).unwrap_or_default()
    }

    /// Start the VictoriaMetrics push loop.
    pub fn start_push_loop(
        self: Arc<Self>,
        config: VictoriaMetricsConfig,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if !config.enabled {
                return;
            }

            info!(
                endpoint = %config.endpoint,
                interval = config.push_interval_secs,
                "Starting VictoriaMetrics push loop"
            );

            let client = reqwest::Client::new();
            let mut tick = interval(Duration::from_secs(config.push_interval_secs));

            loop {
                tick.tick().await;

                let metrics_text = self.gather_text();

                match client
                    .post(&config.endpoint)
                    .header("Content-Type", "text/plain")
                    .body(metrics_text)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if !resp.status().is_success() {
                            error!(
                                status = %resp.status(),
                                "VictoriaMetrics push failed"
                            );
                        } else {
                            debug!("VictoriaMetrics push successful");
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "VictoriaMetrics push error");
                    }
                }
            }
        })
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics collector")
    }
}
