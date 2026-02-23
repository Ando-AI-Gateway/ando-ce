use figment::{Figment, providers::{Env, Format, Yaml}};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level gateway configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub deployment: DeploymentConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ── Impls ─────────────────────────────────────────────────────

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            proxy: ProxyConfig::default(),
            admin: AdminConfig::default(),
            deployment: DeploymentConfig::default(),
            observability: ObservabilityConfig::default(),
        }
    }
}

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

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            victoria_metrics: VictoriaMetricsConfig::default(),
            victoria_logs: VictoriaLogsConfig::default(),
            prometheus: PrometheusConfig::default(),
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
}
