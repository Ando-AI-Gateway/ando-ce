use figment::providers::{Env, Format, Yaml};
use figment::Figment;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Top-level configuration for Ando API Gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndoConfig {
    /// Gateway node ID (auto-generated if not set)
    #[serde(default = "default_node_id")]
    pub node_id: String,

    /// Proxy listener configuration
    #[serde(default)]
    pub proxy: ProxyConfig,

    /// Admin API configuration
    #[serde(default)]
    pub admin: AdminConfig,

    /// etcd configuration
    #[serde(default)]
    pub etcd: EtcdConfig,

    /// Observability configuration
    #[serde(default)]
    pub observability: ObservabilityConfig,

    /// Lua plugin configuration
    #[serde(default)]
    pub lua: LuaConfig,

    /// Deployment mode
    #[serde(default)]
    pub deployment: DeploymentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// HTTP listener address
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,

    /// HTTPS listener address
    #[serde(default = "default_https_addr")]
    pub https_addr: SocketAddr,

    /// Number of worker threads (0 = auto)
    #[serde(default)]
    pub workers: usize,

    /// Enable HTTP/2
    #[serde(default = "default_true")]
    pub http2: bool,

    /// Enable gRPC proxying
    #[serde(default = "default_true")]
    pub grpc: bool,

    /// Enable WebSocket proxying
    #[serde(default = "default_true")]
    pub websocket: bool,

    /// Request body buffer size (bytes)
    #[serde(default = "default_body_buffer_size")]
    pub body_buffer_size: usize,

    /// Upstream connection timeout (milliseconds)
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_ms: u64,

    /// Upstream read timeout (milliseconds)
    #[serde(default = "default_read_timeout")]
    pub read_timeout_ms: u64,

    /// Upstream write timeout (milliseconds)
    #[serde(default = "default_write_timeout")]
    pub write_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    /// Admin API listener address
    #[serde(default = "default_admin_addr")]
    pub addr: SocketAddr,

    /// Admin API key for authentication
    #[serde(default)]
    pub api_key: Option<String>,

    /// Enable Admin API
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// CORS allowed origins
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtcdConfig {
    /// etcd endpoint addresses
    #[serde(default = "default_etcd_endpoints")]
    pub endpoints: Vec<String>,

    /// Key prefix for Ando data
    #[serde(default = "default_etcd_prefix")]
    pub prefix: String,

    /// Connection timeout (milliseconds)
    #[serde(default = "default_etcd_timeout")]
    pub timeout_ms: u64,

    /// Username for etcd auth
    #[serde(default)]
    pub username: Option<String>,

    /// Password for etcd auth
    #[serde(default)]
    pub password: Option<String>,

    /// TLS configuration for etcd
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub ca_cert: PathBuf,
    pub client_cert: Option<PathBuf>,
    pub client_key: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// VictoriaMetrics configuration
    #[serde(default)]
    pub victoria_metrics: VictoriaMetricsConfig,

    /// VictoriaLogs configuration
    #[serde(default)]
    pub victoria_logs: VictoriaLogsConfig,

    /// Prometheus metrics endpoint
    #[serde(default)]
    pub prometheus: PrometheusConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VictoriaMetricsConfig {
    /// Enable VictoriaMetrics push
    #[serde(default)]
    pub enabled: bool,

    /// Remote write endpoint URL
    #[serde(default = "default_vm_endpoint")]
    pub endpoint: String,

    /// Push interval in seconds
    #[serde(default = "default_push_interval")]
    pub push_interval_secs: u64,

    /// Additional labels
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VictoriaLogsConfig {
    /// Enable VictoriaLogs push
    #[serde(default)]
    pub enabled: bool,

    /// JSON stream ingest endpoint
    #[serde(default = "default_vl_endpoint")]
    pub endpoint: String,

    /// Batch size before flush
    #[serde(default = "default_log_batch_size")]
    pub batch_size: usize,

    /// Flush interval in seconds
    #[serde(default = "default_log_flush_interval")]
    pub flush_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusConfig {
    /// Enable Prometheus scrape endpoint
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Metrics path
    #[serde(default = "default_metrics_path")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaConfig {
    /// Directory to search for Lua plugins
    #[serde(default = "default_lua_plugin_dir")]
    pub plugin_dir: PathBuf,

    /// Number of Lua VMs in the pool
    #[serde(default = "default_lua_pool_size")]
    pub pool_size: usize,

    /// Maximum Lua script execution time (milliseconds)
    #[serde(default = "default_lua_timeout")]
    pub timeout_ms: u64,

    /// Maximum memory per Lua VM (bytes, 0 = unlimited)
    #[serde(default = "default_lua_max_memory")]
    pub max_memory: usize,

    /// Additional Lua package paths
    #[serde(default)]
    pub package_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    /// Deployment mode: "standard", "standalone", "edge"
    #[serde(default = "default_mode")]
    pub mode: DeploymentMode,

    /// Standalone config file (used when mode = standalone or edge)
    #[serde(default)]
    pub config_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentMode {
    /// Standard mode with etcd
    Standard,
    /// Standalone mode with file-based config
    Standalone,
    /// Edge mode — minimal footprint, file-based config
    Edge,
}

impl Default for DeploymentMode {
    fn default() -> Self {
        Self::Standard
    }
}

impl AndoConfig {
    /// Load configuration from YAML file + environment variables.
    pub fn load(config_path: Option<&str>) -> anyhow::Result<Self> {
        let mut figment = Figment::new();

        if let Some(path) = config_path {
            figment = figment.merge(Yaml::file(path));
        } else {
            // Try default locations
            for default_path in &["ando.yaml", "/etc/ando/ando.yaml", "config/ando.yaml"] {
                if std::path::Path::new(default_path).exists() {
                    figment = figment.merge(Yaml::file(default_path));
                    break;
                }
            }
        }

        // Environment variables override: ANDO_PROXY__HTTP_ADDR, etc.
        figment = figment.merge(Env::prefixed("ANDO_").split("__"));

        let config: Self = figment.extract()?;
        Ok(config)
    }

    /// Check if running in standalone/edge mode (no etcd required)
    pub fn is_standalone(&self) -> bool {
        matches!(
            self.deployment.mode,
            DeploymentMode::Standalone | DeploymentMode::Edge
        )
    }

    /// Check if running in edge mode
    pub fn is_edge(&self) -> bool {
        self.deployment.mode == DeploymentMode::Edge
    }
}

// Default implementations

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            http_addr: default_http_addr(),
            https_addr: default_https_addr(),
            workers: 0,
            http2: true,
            grpc: true,
            websocket: true,
            body_buffer_size: default_body_buffer_size(),
            connect_timeout_ms: default_connect_timeout(),
            read_timeout_ms: default_read_timeout(),
            write_timeout_ms: default_write_timeout(),
        }
    }
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            addr: default_admin_addr(),
            api_key: None,
            enabled: true,
            cors_origins: vec![],
        }
    }
}

impl Default for EtcdConfig {
    fn default() -> Self {
        Self {
            endpoints: default_etcd_endpoints(),
            prefix: default_etcd_prefix(),
            timeout_ms: default_etcd_timeout(),
            username: None,
            password: None,
            tls: None,
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
            labels: std::collections::HashMap::new(),
        }
    }
}

impl Default for VictoriaLogsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_vl_endpoint(),
            batch_size: default_log_batch_size(),
            flush_interval_secs: default_log_flush_interval(),
        }
    }
}

impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: default_metrics_path(),
        }
    }
}

impl Default for LuaConfig {
    fn default() -> Self {
        Self {
            plugin_dir: default_lua_plugin_dir(),
            pool_size: default_lua_pool_size(),
            timeout_ms: default_lua_timeout(),
            max_memory: default_lua_max_memory(),
            package_path: vec![],
        }
    }
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            mode: DeploymentMode::Standard,
            config_file: None,
        }
    }
}

impl Default for AndoConfig {
    fn default() -> Self {
        Self {
            node_id: default_node_id(),
            proxy: ProxyConfig::default(),
            admin: AdminConfig::default(),
            etcd: EtcdConfig::default(),
            observability: ObservabilityConfig::default(),
            lua: LuaConfig::default(),
            deployment: DeploymentConfig::default(),
        }
    }
}

// Serde default functions
fn default_node_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_http_addr() -> SocketAddr {
    "0.0.0.0:9080".parse().unwrap()
}

fn default_https_addr() -> SocketAddr {
    "0.0.0.0:9443".parse().unwrap()
}

fn default_admin_addr() -> SocketAddr {
    "127.0.0.1:9180".parse().unwrap()
}

fn default_etcd_endpoints() -> Vec<String> {
    vec!["http://127.0.0.1:2379".to_string()]
}

fn default_etcd_prefix() -> String {
    "/ando".to_string()
}

fn default_etcd_timeout() -> u64 {
    5000
}

fn default_vm_endpoint() -> String {
    "http://127.0.0.1:8428/api/v1/write".to_string()
}

fn default_vl_endpoint() -> String {
    "http://127.0.0.1:9428/insert/jsonline".to_string()
}

fn default_push_interval() -> u64 {
    15
}

fn default_log_batch_size() -> usize {
    1000
}

fn default_log_flush_interval() -> u64 {
    5
}

fn default_metrics_path() -> String {
    "/metrics".to_string()
}

fn default_lua_plugin_dir() -> PathBuf {
    PathBuf::from("/etc/ando/plugins")
}

fn default_lua_pool_size() -> usize {
    32
}

fn default_lua_timeout() -> u64 {
    5000
}

fn default_lua_max_memory() -> usize {
    64 * 1024 * 1024 // 64MB
}

fn default_body_buffer_size() -> usize {
    64 * 1024 // 64KB
}

fn default_connect_timeout() -> u64 {
    6000
}

fn default_read_timeout() -> u64 {
    15000
}

fn default_write_timeout() -> u64 {
    15000
}

fn default_true() -> bool {
    true
}

fn default_mode() -> DeploymentMode {
    DeploymentMode::Standard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AndoConfig::default();
        assert_eq!(cfg.proxy.http_addr.port(), 9080);
        assert_eq!(cfg.proxy.https_addr.port(), 9443);
        assert_eq!(cfg.admin.addr.port(), 9180);
        assert!(!cfg.is_standalone());
        assert!(!cfg.is_edge());
    }

    #[test]
    fn test_edge_mode_detection() {
        let mut cfg = AndoConfig::default();
        cfg.deployment.mode = DeploymentMode::Edge;
        assert!(cfg.is_standalone());
        assert!(cfg.is_edge());
    }
}
