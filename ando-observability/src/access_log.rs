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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(upstream: Option<&str>) -> AccessLogEntry {
        AccessLogEntry {
            timestamp: "2024-01-01T00:00:00Z".into(),
            route_id: "route-1".into(),
            client_ip: "192.168.1.1".into(),
            method: "GET".into(),
            uri: "/api/hello".into(),
            response_status: 200,
            latency_ms: 12.5,
            upstream_addr: upstream.map(str::to_string),
        }
    }

    // ── Serialisation ────────────────────────────────────────────

    #[test]
    fn serialises_all_fields() {
        let entry = sample_entry(Some("10.0.0.1:8080"));
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["timestamp"], "2024-01-01T00:00:00Z");
        assert_eq!(json["route_id"], "route-1");
        assert_eq!(json["client_ip"], "192.168.1.1");
        assert_eq!(json["method"], "GET");
        assert_eq!(json["uri"], "/api/hello");
        assert_eq!(json["response_status"], 200);
        assert_eq!(json["latency_ms"], 12.5);
        assert_eq!(json["upstream_addr"], "10.0.0.1:8080");
    }

    #[test]
    fn upstream_addr_none_serialises_to_null() {
        let entry = sample_entry(None);
        let json = serde_json::to_value(&entry).unwrap();
        assert!(json["upstream_addr"].is_null());
    }

    // ── Deserialisation ──────────────────────────────────────────

    #[test]
    fn roundtrip_with_upstream() {
        let src = sample_entry(Some("10.0.0.2:9000"));
        let s = serde_json::to_string(&src).unwrap();
        let dst: AccessLogEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(dst.route_id, src.route_id);
        assert_eq!(dst.response_status, 200);
        assert_eq!(dst.upstream_addr, Some("10.0.0.2:9000".to_string()));
    }

    #[test]
    fn roundtrip_without_upstream() {
        let src = sample_entry(None);
        let s = serde_json::to_string(&src).unwrap();
        let dst: AccessLogEntry = serde_json::from_str(&s).unwrap();
        assert!(dst.upstream_addr.is_none());
    }

    // ── Debug format ─────────────────────────────────────────────

    #[test]
    fn debug_format_does_not_panic() {
        let entry = sample_entry(Some("127.0.0.1:80"));
        let dbg = format!("{entry:?}");
        assert!(dbg.contains("route-1"));
    }

    // ── Status codes ─────────────────────────────────────────────

    #[test]
    fn various_status_codes_serialise_correctly() {
        for status in [200u16, 201, 301, 400, 401, 403, 404, 429, 500, 502, 504] {
            let mut entry = sample_entry(None);
            entry.response_status = status;
            let json = serde_json::to_value(&entry).unwrap();
            assert_eq!(json["response_status"], status);
        }
    }
}
