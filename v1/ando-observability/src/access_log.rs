use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Access log entry format.
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessLogEntry {
    pub timestamp: String,
    pub route_id: String,
    pub service_id: Option<String>,
    pub consumer: Option<String>,
    pub client_ip: String,
    pub method: String,
    pub uri: String,
    pub request_size: u64,
    pub response_status: u16,
    pub response_size: u64,
    pub upstream_addr: Option<String>,
    pub upstream_status: Option<u16>,
    pub latency_ms: f64,
    pub upstream_latency_ms: Option<f64>,
    pub server_addr: String,
    pub request_headers: HashMap<String, String>,
    pub response_headers: HashMap<String, String>,
}
