use serde::{Deserialize, Serialize};

/// Structured access log entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessLogEntry {
    pub timestamp: String,
    pub route_id: String,
    pub client_ip: String,
    pub method: String,
    pub uri: String,
    pub response_status: u16,
    pub latency_ms: f64,
    pub upstream_addr: Option<String>,
}
