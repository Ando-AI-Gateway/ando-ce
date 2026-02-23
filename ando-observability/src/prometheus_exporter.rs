use prometheus::{Encoder, TextEncoder};

/// Render prometheus text exposition format from a registry.
pub fn render_metrics(registry: &prometheus::Registry) -> String {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap_or(());
    String::from_utf8(buffer).unwrap_or_default()
}
