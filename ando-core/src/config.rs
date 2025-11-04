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
