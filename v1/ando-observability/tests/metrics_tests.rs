use ando_observability::metrics::MetricsCollector;
use ando_observability::prometheus_exporter::render_metrics;

// =============================================================================
// MetricsCollector Tests
// =============================================================================

#[test]
fn test_metrics_collector_new() {
    let collector = MetricsCollector::new();
    assert!(collector.is_ok());
}

#[test]
fn test_metrics_collector_default() {
    let collector = MetricsCollector::default();
    // Should not panic
    let _ = collector.gather_text();
}

#[test]
fn test_metrics_record_request() {
    let collector = MetricsCollector::new().unwrap();
    collector.record_request("route-1", "GET", 200, 0.015);
    collector.record_request("route-1", "POST", 201, 0.025);
    collector.record_request("route-2", "GET", 404, 0.005);

    let text = collector.gather_text();
    assert!(text.contains("ando_ando_http_requests_total"));
}

#[test]
fn test_metrics_gather_text() {
    let collector = MetricsCollector::new().unwrap();
    let text = collector.gather_text();
    // Even without any recorded metrics, the exposition should be valid
    assert!(text.is_empty() || text.contains("#")); // Prometheus format has comments
}

#[test]
fn test_metrics_gather_text_with_data() {
    let collector = MetricsCollector::new().unwrap();
    collector.record_request("my-route", "GET", 200, 0.1);

    let text = collector.gather_text();
    assert!(!text.is_empty());
    assert!(text.contains("ando_ando_http_requests_total"));
    assert!(text.contains("my-route"));
}

#[test]
fn test_active_connections_gauge() {
    let collector = MetricsCollector::new().unwrap();

    collector.active_connections.inc();
    collector.active_connections.inc();
    assert_eq!(collector.active_connections.get(), 2);

    collector.active_connections.dec();
    assert_eq!(collector.active_connections.get(), 1);
}

#[test]
fn test_ingress_egress_bytes() {
    let collector = MetricsCollector::new().unwrap();

    collector.ingress_bytes.with_label_values(&["r1"]).inc_by(1024);
    collector.egress_bytes.with_label_values(&["r1"]).inc_by(2048);

    let text = collector.gather_text();
    assert!(text.contains("ando_ando_ingress_bytes_total"));
    assert!(text.contains("ando_ando_egress_bytes_total"));
}

#[test]
fn test_plugin_execution_time() {
    let collector = MetricsCollector::new().unwrap();

    collector
        .plugin_execution_time
        .with_label_values(&["key-auth", "access"])
        .observe(0.001);
    collector
        .plugin_execution_time
        .with_label_values(&["cors", "rewrite"])
        .observe(0.0005);

    let text = collector.gather_text();
    assert!(text.contains("ando_ando_plugin_execution_seconds"));
}

#[test]
fn test_lua_pool_gauges() {
    let collector = MetricsCollector::new().unwrap();

    collector.lua_pool_available.set(30);
    collector.lua_pool_in_use.set(2);

    assert_eq!(collector.lua_pool_available.get(), 30);
    assert_eq!(collector.lua_pool_in_use.get(), 2);
}

#[test]
fn test_upstream_latency() {
    let collector = MetricsCollector::new().unwrap();

    collector
        .upstream_latency
        .with_label_values(&["upstream-1"])
        .observe(0.05);
    collector
        .upstream_latency
        .with_label_values(&["upstream-1"])
        .observe(0.1);

    let text = collector.gather_text();
    assert!(text.contains("ando_ando_upstream_latency_seconds"));
}

// =============================================================================
// Prometheus Exporter Tests
// =============================================================================

#[test]
fn test_render_metrics() {
    let collector = MetricsCollector::new().unwrap();
    collector.record_request("test", "GET", 200, 0.01);

    let text = render_metrics(&collector);
    assert!(!text.is_empty());
    assert!(text.contains("ando_ando_http_requests_total"));
}

#[test]
fn test_render_metrics_empty() {
    let collector = MetricsCollector::new().unwrap();
    let text = render_metrics(&collector);
    // Should be valid (empty or with comments)
    assert!(text.is_empty() || !text.contains("ERROR"));
}

// =============================================================================
// Multiple Records Tests
// =============================================================================

#[test]
fn test_metrics_multiple_routes() {
    let collector = MetricsCollector::new().unwrap();

    for i in 0..100 {
        let route = format!("route-{}", i % 5);
        let method = if i % 2 == 0 { "GET" } else { "POST" };
        let status = if i % 10 == 0 { 500 } else { 200 };
        let duration = (i as f64) * 0.001;
        collector.record_request(&route, method, status, duration);
    }

    let text = collector.gather_text();
    assert!(!text.is_empty());
    // Should contain metric entries for different routes
    assert!(text.contains("route-0"));
    assert!(text.contains("route-4"));
}
