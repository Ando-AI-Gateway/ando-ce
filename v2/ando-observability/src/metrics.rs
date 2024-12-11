use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts, Registry, TextEncoder,
};

/// Metrics collector — all counters are gated behind `enabled`.
///
/// v2 design: When `enabled = false`, the MetricsCollector is a no-op struct.
/// No prometheus Registry is created, no atomic counters are allocated.
/// This eliminates ALL metrics overhead from the data plane hot path.
pub struct MetricsCollector {
    enabled: bool,
    registry: Option<Registry>,
    pub http_requests_total: Option<IntCounterVec>,
    pub http_request_duration: Option<HistogramVec>,
    pub active_connections: Option<IntGauge>,
}

impl MetricsCollector {
    /// Create a new collector. When `enabled = false`, everything is None.
    pub fn new(enabled: bool) -> anyhow::Result<Self> {
        if !enabled {
            return Ok(Self {
                enabled: false,
                registry: None,
                http_requests_total: None,
                http_request_duration: None,
                active_connections: None,
            });
        }

        let registry = Registry::new();

        let http_requests_total = IntCounterVec::new(
            Opts::new("ando_http_requests_total", "Total HTTP requests").namespace("ando"),
            &["route", "method", "status"],
        )?;

        let http_request_duration = HistogramVec::new(
            HistogramOpts::new("ando_http_request_duration_seconds", "Request latency")
                .namespace("ando")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["route"],
        )?;

        let active_connections = IntGauge::new("ando_active_connections", "Active connections")?;

        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(http_request_duration.clone()))?;
        registry.register(Box::new(active_connections.clone()))?;

        Ok(Self {
            enabled: true,
            registry: Some(registry),
            http_requests_total: Some(http_requests_total),
            http_request_duration: Some(http_request_duration),
            active_connections: Some(active_connections),
        })
    }

    /// Record a request (no-op when disabled).
    #[inline]
    pub fn record_request(&self, route: &str, method: &str, status: u16, duration_secs: f64) {
        if !self.enabled {
            return;
        }
        if let Some(ref counter) = self.http_requests_total {
            let mut buf = itoa::Buffer::new();
            let status_str = buf.format(status);
            counter.with_label_values(&[route, method, status_str]).inc();
        }
        if let Some(ref hist) = self.http_request_duration {
            hist.with_label_values(&[route]).observe(duration_secs);
        }
    }

    /// Render prometheus text exposition format.
    pub fn render(&self) -> String {
        if let Some(ref registry) = self.registry {
            let encoder = TextEncoder::new();
            let metric_families = registry.gather();
            let mut buffer = Vec::new();
            encoder.encode(&metric_families, &mut buffer).unwrap_or(());
            String::from_utf8(buffer).unwrap_or_default()
        } else {
            String::new()
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}
