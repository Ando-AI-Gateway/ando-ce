use crate::metrics::MetricsCollector;

/// Prometheus exposition endpoint handler.
///
/// Returns metrics in Prometheus text format for scraping.
pub fn render_metrics(collector: &MetricsCollector) -> String {
    collector.gather_text()
}
