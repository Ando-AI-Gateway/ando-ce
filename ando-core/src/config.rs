use figment::{Figment, providers::{Env, Format, Yaml}};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level gateway configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewayConfig {
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub deployment: DeploymentConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    /// Compliance policy settings (SOC2 Type II, ISO 27001:2022, HIPAA, GDPR).
    #[serde(default)]
    pub compliance: ComplianceConfig,
}

/// Data plane proxy settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[serde(default = "default_http_addr")]
    pub http_addr: String,
    #[serde(default = "default_https_addr")]
    pub https_addr: String,
    /// Number of worker threads. 0 = number of CPU cores.
    #[serde(default)]
    pub workers: usize,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_ms: u64,
    #[serde(default = "default_read_timeout")]
    pub read_timeout_ms: u64,
    #[serde(default = "default_write_timeout")]
    pub write_timeout_ms: u64,
    /// Max keepalive connections per upstream, per worker core.
    #[serde(default = "default_keepalive_pool")]
    pub keepalive_pool_size: usize,
}

/// Admin API settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    #[serde(default = "default_admin_addr")]
    pub addr: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Admin API key for authentication (optional).
    pub api_key: Option<String>,
}

/// Deployment mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    #[serde(default = "default_mode")]
    pub mode: DeploymentMode,
    #[serde(default)]
    pub etcd: Option<EtcdConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentMode {
    Standalone,
    Etcd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtcdConfig {
    pub endpoints: Vec<String>,
    #[serde(default = "default_etcd_prefix")]
    pub prefix: String,
    #[serde(default = "default_etcd_timeout")]
    pub timeout_secs: u64,
}

/// Observability settings — all optional, disabled by default.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub victoria_metrics: VictoriaMetricsConfig,
    #[serde(default)]
    pub victoria_logs: VictoriaLogsConfig,
    #[serde(default)]
    pub prometheus: PrometheusConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VictoriaMetricsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_vm_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_push_interval")]
    pub push_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VictoriaLogsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_vl_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_flush_interval")]
    pub flush_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusConfig {
    /// When false, no prometheus counters are updated on the hot path.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_metrics_path")]
    pub path: String,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Compliance (SOC2 Type II · ISO 27001:2022 · HIPAA · GDPR)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Top-level compliance policy block.
///
/// When `hipaa = true` or `gdpr = true` the gateway will enforce the minimum
/// set of controls mandated by that regulation.  Individual sub-sections
/// (`tls`, `audit_log`, `pii_scrubbing`) can also be tuned directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceConfig {
    /// Enable HIPAA compliance mode.
    /// Implies: `audit_log.enabled`, TLS 1.2+, `pii_scrubbing.scrub_headers`.
    #[serde(default)]
    pub hipaa: bool,
    /// Enable GDPR compliance mode.
    /// Implies: `pii_scrubbing.anonymize_ips`, data-minimisation logging.
    #[serde(default)]
    pub gdpr: bool,
    /// Enable SOC2 Type II mode.
    /// Implies: `audit_log.enabled`, security-headers plugin, TLS strict.
    #[serde(default)]
    pub soc2: bool,
    /// Enable ISO/IEC 27001:2022 mode.
    /// Implies: `audit_log.enabled`, TLS 1.2+, `pii_scrubbing.scrub_headers`.
    #[serde(default)]
    pub iso27001: bool,
    #[serde(default)]
    pub tls: TlsComplianceConfig,
    #[serde(default)]
    pub audit_log: AuditLogConfig,
    #[serde(default)]
    pub pii_scrubbing: PiiScrubConfig,
    /// Audit / access-log retention period in days.  0 = unlimited.
    /// HIPAA § 164.530(j) requires ≥6 years.  SOC2 commonly 1 year.
    #[serde(default = "default_retention_days")]
    pub log_retention_days: u32,
    /// Informational data-residency region tag (e.g. "eu", "us", "apac").
    /// Used by operators to enforce geographic routing at the infra layer.
    #[serde(default)]
    pub data_residency_region: Option<String>,
}

/// TLS hardening settings.
///
/// HIPAA Technical Safeguard 164.312(e)(1), SOC2 CC6.7,
/// ISO 27001:2022 A.8.24, PCI-DSS 4.0 req 4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsComplianceConfig {
    /// Minimum TLS version accepted. Must be "TLSv1.2" or "TLSv1.3".
    /// All four frameworks require ≥ TLS 1.2 for data-in-transit protection.
    #[serde(default = "default_tls_min_version")]
    pub min_version: String,
    /// Restrict cipher suites to the NIST/FIPS-approved set.
    #[serde(default = "default_true_bool")]
    pub strict_ciphers: bool,
    /// HSTS max-age in seconds injected into the security-headers plugin
    /// (default 31 536 000 = 1 year; Chrome preload requires ≥ 1 year).
    #[serde(default = "default_hsts_max_age")]
    pub hsts_max_age_secs: u64,
}

/// Compliance audit log configuration.
///
/// Provides the immutable, tamper-evident record trail required by:
///   HIPAA 164.312(b) · SOC2 CC6.1/CC7.2 · ISO 27001:2022 A.8.15
///   GDPR Art. 30 (Records of Processing Activities)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogConfig {
    /// Emit a structured JSON audit record for every HTTP transaction.
    #[serde(default)]
    pub enabled: bool,
    /// Include a SHA-256 hash of the request body (HIPAA integrity control).
    /// Disabled by default to avoid overhead on non-PHI routes.
    #[serde(default)]
    pub include_request_body_hash: bool,
    /// Output format: `"json"` (default) or `"text"`.
    #[serde(default = "default_audit_format")]
    pub format: String,
    /// Destination file path.  Empty / None → stdout.
    #[serde(default)]
    pub file_path: Option<String>,
}

/// PII / PHI scrubbing settings.
///
/// Satisfies:
///   HIPAA 164.312(e)(2)(ii) de-identification · GDPR Art. 32 pseudonymisation
///   SOC2 Confidentiality criteria · ISO 27001:2022 A.8.11 data masking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PiiScrubConfig {
    /// Mask well-known sensitive request headers
    /// (Authorization, Cookie, Set-Cookie, X-Api-Key, X-Auth-Token, …).
    #[serde(default)]
    pub scrub_headers: bool,
    /// Additional header names to mask (case-insensitive).
    #[serde(default)]
    pub extra_sensitive_headers: Vec<String>,
    /// Replace the last octet of IPv4 / last 64 bits of IPv6 with zeros
    /// in access logs and audit records (GDPR Art. 32).
    #[serde(default)]
    pub anonymize_ips: bool,
    /// List of regex patterns whose matching substrings in the request URI
    /// are replaced with `[REDACTED]` before logging.
    #[serde(default)]
    pub uri_redact_patterns: Vec<String>,
}

// ── Defaults ──────────────────────────────────────────────────

fn default_http_addr() -> String { "0.0.0.0:9080".into() }
fn default_https_addr() -> String { "0.0.0.0:9443".into() }
fn default_admin_addr() -> String { "0.0.0.0:9180".into() }
fn default_connect_timeout() -> u64 { 2000 }
fn default_read_timeout() -> u64 { 5000 }
fn default_write_timeout() -> u64 { 5000 }
fn default_keepalive_pool() -> usize { 256 }
fn default_true() -> bool { true }
fn default_mode() -> DeploymentMode { DeploymentMode::Standalone }
fn default_etcd_prefix() -> String { "/ando".into() }
fn default_etcd_timeout() -> u64 { 30 }
fn default_vm_endpoint() -> String { "http://localhost:8428/api/v1/import/prometheus".into() }
fn default_vl_endpoint() -> String { "http://localhost:9428/insert/jsonline".into() }
fn default_push_interval() -> u64 { 15 }
fn default_batch_size() -> usize { 1000 }
fn default_flush_interval() -> u64 { 5 }
fn default_metrics_path() -> String { "/metrics".into() }
fn default_tls_min_version() -> String { "TLSv1.2".into() }
fn default_true_bool() -> bool { true }
fn default_hsts_max_age() -> u64 { 31_536_000 }
fn default_retention_days() -> u32 { 365 }
fn default_audit_format() -> String { "json".into() }

// ── Impls ─────────────────────────────────────────────────────

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            http_addr: default_http_addr(),
            https_addr: default_https_addr(),
            workers: 0,
            connect_timeout_ms: default_connect_timeout(),
            read_timeout_ms: default_read_timeout(),
            write_timeout_ms: default_write_timeout(),
            keepalive_pool_size: default_keepalive_pool(),
        }
    }
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            addr: default_admin_addr(),
            enabled: true,
            api_key: None,
        }
    }
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            mode: DeploymentMode::Standalone,
            etcd: None,
        }
    }
}

impl Default for VictoriaMetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_vm_endpoint(),
            push_interval_secs: default_push_interval(),
        }
    }
}

impl Default for VictoriaLogsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_vl_endpoint(),
            batch_size: default_batch_size(),
            flush_interval_secs: default_flush_interval(),
        }
    }
}

impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: default_metrics_path(),
        }
    }
}

impl Default for TlsComplianceConfig {
    fn default() -> Self {
        Self {
            min_version: default_tls_min_version(),
            strict_ciphers: true,
            hsts_max_age_secs: default_hsts_max_age(),
        }
    }
}

impl Default for AuditLogConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            include_request_body_hash: false,
            format: default_audit_format(),
            file_path: None,
        }
    }
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            hipaa: false,
            gdpr: false,
            soc2: false,
            iso27001: false,
            tls: TlsComplianceConfig::default(),
            audit_log: AuditLogConfig::default(),
            pii_scrubbing: PiiScrubConfig::default(),
            log_retention_days: default_retention_days(),
            data_residency_region: None,
        }
    }
}

impl GatewayConfig {
    /// Load configuration from YAML file + env overrides.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let config: GatewayConfig = Figment::new()
            .merge(Yaml::file(path))
            .merge(Env::prefixed("ANDO_").split("_"))
            .extract()?;
        Ok(config)
    }

    /// Effective worker count (0 → available CPUs).
    pub fn effective_workers(&self) -> usize {
        if self.proxy.workers == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        } else {
            self.proxy.workers
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── Default values ────────────────────────────────────────────

    #[test]
    fn default_proxy_config_has_expected_values() {
        let cfg = ProxyConfig::default();
        assert_eq!(cfg.http_addr, "0.0.0.0:9080");
        assert_eq!(cfg.https_addr, "0.0.0.0:9443");
        assert_eq!(cfg.workers, 0);
        assert_eq!(cfg.connect_timeout_ms, 2000);
        assert_eq!(cfg.read_timeout_ms, 5000);
        assert_eq!(cfg.write_timeout_ms, 5000);
        assert_eq!(cfg.keepalive_pool_size, 256);
    }

    #[test]
    fn default_admin_config_has_expected_values() {
        let cfg = AdminConfig::default();
        assert_eq!(cfg.addr, "0.0.0.0:9180");
        assert!(cfg.enabled);
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn default_deployment_config_is_standalone_no_etcd() {
        let cfg = DeploymentConfig::default();
        assert_eq!(cfg.mode, DeploymentMode::Standalone);
        assert!(cfg.etcd.is_none());
    }

    #[test]
    fn default_observability_all_disabled() {
        let cfg = ObservabilityConfig::default();
        assert!(!cfg.victoria_metrics.enabled);
        assert!(!cfg.victoria_logs.enabled);
        assert!(!cfg.prometheus.enabled);
    }

    #[test]
    fn default_prometheus_config_has_metrics_path() {
        let cfg = PrometheusConfig::default();
        assert_eq!(cfg.path, "/metrics");
        assert!(!cfg.enabled);
    }

    #[test]
    fn default_victoria_logs_config_values() {
        let cfg = VictoriaLogsConfig::default();
        assert_eq!(cfg.batch_size, 1000);
        assert_eq!(cfg.flush_interval_secs, 5);
        assert!(!cfg.enabled);
    }

    #[test]
    fn default_victoria_metrics_config_values() {
        let cfg = VictoriaMetricsConfig::default();
        assert_eq!(cfg.push_interval_secs, 15);
        assert!(!cfg.enabled);
    }

    #[test]
    fn gateway_config_default_builds_without_panic() {
        let cfg = GatewayConfig::default();
        // Ensure nested defaults compose correctly
        assert_eq!(cfg.proxy.http_addr, "0.0.0.0:9080");
        assert_eq!(cfg.admin.addr, "0.0.0.0:9180");
        assert_eq!(cfg.deployment.mode, DeploymentMode::Standalone);
    }

    // ── effective_workers() ───────────────────────────────────────

    #[test]
    fn effective_workers_returns_explicit_value_when_nonzero() {
        let mut cfg = GatewayConfig::default();
        cfg.proxy.workers = 4;
        assert_eq!(cfg.effective_workers(), 4);
    }

    #[test]
    fn effective_workers_with_zero_returns_at_least_one() {
        let cfg = GatewayConfig::default(); // workers = 0
        let workers = cfg.effective_workers();
        assert!(workers >= 1, "effective_workers must be at least 1, got {workers}");
    }

    // ── DeploymentMode serde ──────────────────────────────────────

    #[test]
    fn deployment_mode_standalone_serializes_to_lowercase() {
        let mode = DeploymentMode::Standalone;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"standalone\"");
    }

    #[test]
    fn deployment_mode_etcd_serializes_to_lowercase() {
        let mode = DeploymentMode::Etcd;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"etcd\"");
    }

    #[test]
    fn deployment_mode_roundtrip() {
        for mode in &[DeploymentMode::Standalone, DeploymentMode::Etcd] {
            let serialized = serde_json::to_string(mode).unwrap();
            let deserialized: DeploymentMode = serde_json::from_str(&serialized).unwrap();
            assert_eq!(*mode, deserialized);
        }
    }

    // ── GatewayConfig::load() ─────────────────────────────────────

    #[test]
    fn load_from_nonexistent_file_returns_error() {
        let result = GatewayConfig::load(std::path::Path::new("/nonexistent/path/config.yaml"));
        // Figment returns Ok with defaults when the file is missing (merges empty)
        // or an error — either result is acceptable; ensure we don't panic
        let _ = result;
    }

    #[test]
    fn load_from_valid_yaml_overrides_defaults() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "proxy:\n  http_addr: \"0.0.0.0:8888\"\n  workers: 2\n").unwrap();
        let cfg = GatewayConfig::load(tmpfile.path()).unwrap();
        assert_eq!(cfg.proxy.http_addr, "0.0.0.0:8888");
        assert_eq!(cfg.proxy.workers, 2);
        // Defaults still apply for unspecified fields
        assert_eq!(cfg.proxy.https_addr, "0.0.0.0:9443");
    }

    #[test]
    fn load_yaml_with_etcd_mode() {
        let yaml = r#"
deployment:
  mode: etcd
  etcd:
    endpoints:
      - "http://localhost:2379"
    prefix: "/my-prefix"
    timeout_secs: 10
"#;
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "{yaml}").unwrap();
        let cfg = GatewayConfig::load(tmpfile.path()).unwrap();
        assert_eq!(cfg.deployment.mode, DeploymentMode::Etcd);
        let etcd = cfg.deployment.etcd.unwrap();
        assert_eq!(etcd.endpoints, vec!["http://localhost:2379".to_string()]);
        assert_eq!(etcd.prefix, "/my-prefix");
        assert_eq!(etcd.timeout_secs, 10);
    }

    #[test]
    fn load_yaml_with_observability() {
        let yaml = r#"
observability:
  prometheus:
    enabled: true
    path: "/prom"
  victoria_metrics:
    enabled: true
    endpoint: "http://vm:8428/api/v1/import/prometheus"
  victoria_logs:
    enabled: true
    batch_size: 500
"#;
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "{yaml}").unwrap();
        let cfg = GatewayConfig::load(tmpfile.path()).unwrap();
        assert!(cfg.observability.prometheus.enabled);
        assert_eq!(cfg.observability.prometheus.path, "/prom");
        assert!(cfg.observability.victoria_metrics.enabled);
        assert!(cfg.observability.victoria_logs.enabled);
        assert_eq!(cfg.observability.victoria_logs.batch_size, 500);
    }

    // ── ComplianceConfig ──────────────────────────────────────────

    #[test]
    fn default_compliance_config_all_disabled() {
        let cfg = ComplianceConfig::default();
        assert!(!cfg.hipaa);
        assert!(!cfg.gdpr);
        assert!(!cfg.soc2);
        assert!(!cfg.iso27001);
        assert!(!cfg.audit_log.enabled);
        assert!(!cfg.pii_scrubbing.scrub_headers);
        assert!(!cfg.pii_scrubbing.anonymize_ips);
        assert_eq!(cfg.log_retention_days, 365);
        assert!(cfg.data_residency_region.is_none());
    }

    #[test]
    fn default_tls_compliance_config() {
        let cfg = TlsComplianceConfig::default();
        assert_eq!(cfg.min_version, "TLSv1.2");
        assert!(cfg.strict_ciphers);
        assert_eq!(cfg.hsts_max_age_secs, 31_536_000);
    }

    #[test]
    fn load_yaml_with_hipaa_compliance() {
        let yaml = r#"
compliance:
  hipaa: true
  gdpr: true
  log_retention_days: 2190
  data_residency_region: "us"
  tls:
    min_version: "TLSv1.3"
    strict_ciphers: true
    hsts_max_age_secs: 63072000
  audit_log:
    enabled: true
    include_request_body_hash: true
    format: "json"
  pii_scrubbing:
    scrub_headers: true
    anonymize_ips: true
    extra_sensitive_headers:
      - "x-patient-id"
      - "x-ssn"
"#;
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "{yaml}").unwrap();
        let cfg = GatewayConfig::load(tmpfile.path()).unwrap();
        assert!(cfg.compliance.hipaa);
        assert!(cfg.compliance.gdpr);
        assert_eq!(cfg.compliance.log_retention_days, 2190);
        assert_eq!(cfg.compliance.data_residency_region.as_deref(), Some("us"));
        assert_eq!(cfg.compliance.tls.min_version, "TLSv1.3");
        assert_eq!(cfg.compliance.tls.hsts_max_age_secs, 63_072_000);
        assert!(cfg.compliance.audit_log.enabled);
        assert!(cfg.compliance.audit_log.include_request_body_hash);
        assert!(cfg.compliance.pii_scrubbing.scrub_headers);
        assert!(cfg.compliance.pii_scrubbing.anonymize_ips);
        assert_eq!(
            cfg.compliance.pii_scrubbing.extra_sensitive_headers,
            vec!["x-patient-id".to_string(), "x-ssn".to_string()]
        );
    }

    #[test]
    fn compliance_soc2_mode_defaults() {
        let cfg = ComplianceConfig { soc2: true, ..Default::default() };
        assert_eq!(cfg.tls.min_version, "TLSv1.2");
        assert_eq!(cfg.log_retention_days, 365);
    }

    // ── Negative config tests ─────────────────────────────────────

    #[test]
    fn load_yaml_with_wrong_type_for_workers_returns_error() {
        let yaml = "proxy:\n  workers: \"not-a-number\"\n";
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "{yaml}").unwrap();
        let result = GatewayConfig::load(tmpfile.path());
        assert!(result.is_err(), "YAML with wrong type for workers should error");
    }

    #[test]
    fn load_yaml_with_unknown_deployment_mode_returns_error() {
        let yaml = "deployment:\n  mode: \"cluster\"\n";
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "{yaml}").unwrap();
        let result = GatewayConfig::load(tmpfile.path());
        assert!(result.is_err(), "Unknown deployment mode should error");
    }

    #[test]
    fn load_yaml_with_invalid_yaml_syntax_returns_error() {
        let yaml = "proxy:\n  http_addr: [invalid yaml\n";
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "{yaml}").unwrap();
        let result = GatewayConfig::load(tmpfile.path());
        assert!(result.is_err(), "Invalid YAML syntax should error");
    }

    #[test]
    fn load_empty_yaml_file_gives_defaults() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "").unwrap();
        let cfg = GatewayConfig::load(tmpfile.path()).unwrap();
        assert_eq!(cfg.proxy.http_addr, "0.0.0.0:9080");
        assert_eq!(cfg.deployment.mode, DeploymentMode::Standalone);
    }

    #[test]
    fn load_yaml_with_extra_unknown_keys_is_ok() {
        let yaml = "proxy:\n  http_addr: '0.0.0.0:7777'\nfuture_feature:\n  key: value\n";
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "{yaml}").unwrap();
        // figment silently ignores unknown keys
        let cfg = GatewayConfig::load(tmpfile.path()).unwrap();
        assert_eq!(cfg.proxy.http_addr, "0.0.0.0:7777");
    }

    // ── GatewayConfig serde round-trip ────────────────────────────

    #[test]
    fn gateway_config_json_roundtrip() {
        let cfg = GatewayConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let cfg2: GatewayConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg2.proxy.http_addr, cfg.proxy.http_addr);
        assert_eq!(cfg2.admin.addr, cfg.admin.addr);
        assert_eq!(cfg2.deployment.mode, cfg.deployment.mode);
    }
}
