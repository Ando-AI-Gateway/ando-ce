//! Compliance audit log entry.
//!
//! Provides the structured, tamper-evident audit trail required by:
//!
//! | Standard              | Control                                              |
//! |-----------------------|------------------------------------------------------|
//! | HIPAA                 | 164.312(b) – Audit Controls                          |
//! | SOC2 Type II          | CC6.1 Logical Access / CC7.2 Incident Monitoring     |
//! | ISO/IEC 27001:2022    | A.8.15 – Logging / A.8.16 – Monitoring               |
//! | GDPR                  | Art. 30 – Records of Processing Activities           |
//!
//! Every field is serialisable to JSON so records can be shipped to any
//! SIEM (Splunk, Elastic, VictoriaLogs, Datadog, …).

use chrono::Utc;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────

/// Whether the gateway allowed or denied the transaction.
///
/// Serialised as `"ALLOW"` / `"DENY"` for easy SIEM filtering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum AuditOutcome {
    /// Request was forwarded to the upstream service.
    Allow,
    /// Request was rejected by a policy (auth, rate-limit, IP block, …).
    Deny,
}

/// A single compliance audit record, written once per HTTP transaction.
///
/// Instances are normally created via [`AuditLogEntry::new`] and then filled
/// in by the proxy worker before being serialised to JSON.
///
/// # PII / PHI notice
/// The fields `client_ip` and `uri` may contain personal data.  Callers
/// **must** run them through the PII scrubber
/// (`ando_observability::pii_scrubber`) before constructing this struct when
/// `compliance.pii_scrubbing.{anonymize_ips,scrub_headers}` are enabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    // ── Identity ─────────────────────────────────────────────────
    /// ISO-8601 UTC timestamp (RFC 3339).
    pub timestamp: String,
    /// Unique request identifier, propagated as `X-Request-Id`.
    /// Ties together access-log, audit-log, and distributed traces.
    pub request_id: String,
    /// Authenticated consumer identifier (username / client-id).
    /// `None` when the route has no authentication plugin.
    pub consumer_id: Option<String>,
    /// Distributed trace identifier supplied by the upstream or set by the
    /// gateway (value of `X-Trace-Id` / `traceparent`).
    pub trace_id: Option<String>,

    // ── Request ───────────────────────────────────────────────────
    /// Route identifier matched by the router.
    pub route_id: String,
    /// HTTP method (uppercase: `GET`, `POST`, …).
    pub method: String,
    /// Request URI path + query string.
    /// Query-parameter values matching `pii_scrubbing.uri_redact_patterns`
    /// are replaced with `[REDACTED]`.
    pub uri: String,

    // ── Response ──────────────────────────────────────────────────
    /// HTTP response status code returned to the client.
    pub response_status: u16,
    /// End-to-end latency in milliseconds (request received → response sent).
    pub duration_ms: f64,

    // ── Outcome & policy ─────────────────────────────────────────
    /// Whether the request was allowed or denied by the plugin pipeline.
    pub outcome: AuditOutcome,
    /// Name of the plugin that issued the deny decision.
    /// `None` when `outcome = Allow`.
    pub deny_plugin: Option<String>,
    /// Human-readable reason for a deny decision
    /// (e.g. `"rate limit exceeded"`, `"invalid JWT"`, `"IP blocked"`).
    pub deny_reason: Option<String>,

    // ── Network ───────────────────────────────────────────────────
    /// Client IP address.
    /// When `compliance.pii_scrubbing.anonymize_ips = true` this is
    /// pseudonymised: `192.168.1.42` → `192.168.1.0`.
    pub client_ip: String,

    // ── Data-integrity ────────────────────────────────────────────
    /// `true` when at least one field in this record was scrubbed or
    /// anonymised by the PII scrubber.
    pub pii_scrubbed: bool,
    /// SHA-256 hex digest of the raw request body.
    /// Populated only when `compliance.audit_log.include_request_body_hash = true`.
    /// Provides HIPAA integrity control and non-repudiation evidence.
    pub request_body_hash: Option<String>,
}

impl AuditLogEntry {
    /// Create a minimal entry with only the route ID; fill remaining fields
    /// before serialising.
    pub fn new(route_id: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now().to_rfc3339(),
            request_id: String::new(),
            consumer_id: None,
            trace_id: None,
            route_id: route_id.into(),
            method: String::new(),
            uri: String::new(),
            response_status: 0,
            duration_ms: 0.0,
            outcome: AuditOutcome::Allow,
            deny_plugin: None,
            deny_reason: None,
            client_ip: String::new(),
            pii_scrubbed: false,
            request_body_hash: None,
        }
    }

    /// Convenience: mark the entry as denied.
    pub fn deny(&mut self, plugin: impl Into<String>, reason: impl Into<String>) {
        self.outcome = AuditOutcome::Deny;
        self.deny_plugin = Some(plugin.into());
        self.deny_reason = Some(reason.into());
    }

    /// Serialise to a compact JSON line suitable for log shipping.
    pub fn to_json_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// ─────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AuditLogEntry {
        let mut e = AuditLogEntry::new("route-hipaa-1");
        e.request_id = "req-abc-123".into();
        e.consumer_id = Some("alice".into());
        e.trace_id = Some("trace-xyz".into());
        e.method = "POST".into();
        e.uri = "/api/patients".into();
        e.response_status = 200;
        e.duration_ms = 4.2;
        e.client_ip = "10.0.0.1".into();
        e
    }

    // ── Construction ─────────────────────────────────────────────

    #[test]
    fn new_sets_timestamp_and_defaults() {
        let e = AuditLogEntry::new("r1");
        assert_eq!(e.route_id, "r1");
        assert_eq!(e.outcome, AuditOutcome::Allow);
        assert!(e.deny_plugin.is_none());
        assert!(e.deny_reason.is_none());
        assert!(!e.pii_scrubbed);
        assert!(e.request_body_hash.is_none());
        // Timestamp must be non-empty ISO-8601
        assert!(e.timestamp.contains('T'));
    }

    #[test]
    fn deny_sets_outcome_and_reason() {
        let mut e = AuditLogEntry::new("r1");
        e.deny("rate-limiting", "too many requests");
        assert_eq!(e.outcome, AuditOutcome::Deny);
        assert_eq!(e.deny_plugin.as_deref(), Some("rate-limiting"));
        assert_eq!(e.deny_reason.as_deref(), Some("too many requests"));
    }

    // ── Serialisation ────────────────────────────────────────────

    #[test]
    fn serialises_allow_outcome_as_uppercase() {
        let e = sample();
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["outcome"], "ALLOW");
    }

    #[test]
    fn serialises_deny_outcome_as_uppercase() {
        let mut e = sample();
        e.deny("jwt-auth", "invalid token");
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["outcome"], "DENY");
        assert_eq!(json["deny_plugin"], "jwt-auth");
        assert_eq!(json["deny_reason"], "invalid token");
    }

    #[test]
    fn serialises_all_required_compliance_fields() {
        let e = sample();
        let json = serde_json::to_value(&e).unwrap();
        // Fields required by HIPAA 164.312(b)
        assert!(json.get("timestamp").is_some());
        assert!(json.get("request_id").is_some());
        assert!(json.get("consumer_id").is_some());
        assert!(json.get("route_id").is_some());
        assert!(json.get("method").is_some());
        assert!(json.get("uri").is_some());
        assert!(json.get("response_status").is_some());
        assert!(json.get("outcome").is_some());
        assert!(json.get("client_ip").is_some());
        assert!(json.get("duration_ms").is_some());
    }

    #[test]
    fn optional_fields_serialise_as_null_when_absent() {
        let e = AuditLogEntry::new("r2");
        let json = serde_json::to_value(&e).unwrap();
        assert!(json["consumer_id"].is_null());
        assert!(json["trace_id"].is_null());
        assert!(json["deny_plugin"].is_null());
        assert!(json["deny_reason"].is_null());
        assert!(json["request_body_hash"].is_null());
    }

    #[test]
    fn to_json_line_produces_valid_json() {
        let e = sample();
        let line = e.to_json_line();
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["route_id"], "route-hipaa-1");
    }

    #[test]
    fn roundtrip_preserves_all_fields() {
        let mut e = sample();
        e.pii_scrubbed = true;
        e.request_body_hash = Some("abcdef1234567890".into());
        let json = serde_json::to_string(&e).unwrap();
        let e2: AuditLogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e2.consumer_id, e.consumer_id);
        assert!(e2.pii_scrubbed);
        assert_eq!(e2.request_body_hash.as_deref(), Some("abcdef1234567890"));
    }

    // ── HIPAA / GDPR field checks ─────────────────────────────────

    #[test]
    fn pii_scrubbed_flag_is_false_by_default() {
        let e = AuditLogEntry::new("r3");
        assert!(!e.pii_scrubbed);
    }

    #[test]
    fn body_hash_field_can_store_sha256_hex() {
        let mut e = AuditLogEntry::new("r4");
        let hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        e.request_body_hash = Some(hash.to_string());
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["request_body_hash"], hash);
    }

    #[test]
    fn deny_method_is_idempotent_on_second_call() {
        let mut e = AuditLogEntry::new("r5");
        e.deny("plugin-a", "first reason");
        e.deny("plugin-b", "second reason");
        // Last call wins — the record reflects the final decision
        assert_eq!(e.deny_plugin.as_deref(), Some("plugin-b"));
        assert_eq!(e.outcome, AuditOutcome::Deny);
    }
}
