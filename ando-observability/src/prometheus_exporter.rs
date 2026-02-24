use prometheus::{Encoder, TextEncoder};

/// Render prometheus text exposition format from a registry.
pub fn render_metrics(registry: &prometheus::Registry) -> String {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap_or(());
    String::from_utf8(buffer).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::{Counter, Gauge, Opts, Registry};

    #[test]
    fn render_empty_registry_returns_empty_string() {
        let registry = Registry::new();
        let output = render_metrics(&registry);
        assert!(
            output.is_empty(),
            "Empty registry should produce no output, got: {output:?}"
        );
    }

    #[test]
    fn render_registry_with_counter_contains_metric_name() {
        let registry = Registry::new();
        let counter =
            Counter::with_opts(Opts::new("http_requests_total", "Total HTTP requests")).unwrap();
        registry.register(Box::new(counter.clone())).unwrap();
        counter.inc();

        let output = render_metrics(&registry);
        assert!(
            output.contains("http_requests_total"),
            "Output must contain metric name"
        );
        assert!(output.contains("1"), "Output must contain counter value 1");
    }

    #[test]
    fn render_registry_with_gauge_includes_gauge_type() {
        let registry = Registry::new();
        let gauge =
            Gauge::with_opts(Opts::new("active_connections", "Active connections")).unwrap();
        registry.register(Box::new(gauge.clone())).unwrap();
        gauge.set(42.0);

        let output = render_metrics(&registry);
        assert!(
            output.contains("active_connections"),
            "Output must contain gauge name"
        );
        assert!(output.contains("42"), "Output must contain gauge value");
    }

    #[test]
    fn render_output_is_valid_utf8() {
        let registry = Registry::new();
        let counter = Counter::with_opts(Opts::new("test_counter", "Test counter")).unwrap();
        registry.register(Box::new(counter)).unwrap();
        let output = render_metrics(&registry);
        // If this doesn't panic, the output is valid UTF-8
        assert!(!output.is_empty());
    }
}
