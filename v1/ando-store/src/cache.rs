use ando_core::consumer::Consumer;
use ando_core::plugin_config::PluginConfig;
use ando_core::route::Route;
use ando_core::service::Service;
use ando_core::ssl::SslCert;
use ando_core::upstream::Upstream;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::debug;

/// In-memory configuration cache for zero-latency lookups.
///
/// The cache is populated from etcd on startup and kept in sync
/// via the ConfigWatcher. All proxy decisions read from this cache
/// rather than hitting etcd on every request.
#[derive(Clone)]
pub struct ConfigCache {
    pub routes: Arc<DashMap<String, Route>>,
    pub services: Arc<DashMap<String, Service>>,
    pub upstreams: Arc<DashMap<String, Upstream>>,
    pub consumers: Arc<DashMap<String, Consumer>>,
    pub ssl_certs: Arc<DashMap<String, SslCert>>,
    pub plugin_configs: Arc<DashMap<String, PluginConfig>>,
}

impl ConfigCache {
    pub fn new() -> Self {
        Self {
            routes: Arc::new(DashMap::new()),
            services: Arc::new(DashMap::new()),
            upstreams: Arc::new(DashMap::new()),
            consumers: Arc::new(DashMap::new()),
            ssl_certs: Arc::new(DashMap::new()),
            plugin_configs: Arc::new(DashMap::new()),
        }
    }

    /// Apply a change event from etcd.
    pub fn apply_change(&self, resource_type: &str, id: &str, value: Option<&str>) {
        match resource_type {
            "routes" => {
                if let Some(val) = value {
                    match serde_json::from_str::<Route>(val) {
                        Ok(route) => {
                            self.routes.insert(id.to_string(), route);
                            debug!(resource = "route", id = id, "Cache updated");
                        }
                        Err(e) => tracing::error!(error = %e, "Failed to deserialize route"),
                    }
                } else {
                    self.routes.remove(id);
                    debug!(resource = "route", id = id, "Cache removed");
                }
            }
            "services" => {
                if let Some(val) = value {
                    if let Ok(service) = serde_json::from_str::<Service>(val) {
                        self.services.insert(id.to_string(), service);
                    }
                } else {
                    self.services.remove(id);
                }
            }
            "upstreams" => {
                if let Some(val) = value {
                    if let Ok(upstream) = serde_json::from_str::<Upstream>(val) {
                        self.upstreams.insert(id.to_string(), upstream);
                    }
                } else {
                    self.upstreams.remove(id);
                }
            }
            "consumers" => {
                if let Some(val) = value {
                    if let Ok(consumer) = serde_json::from_str::<Consumer>(val) {
                        self.consumers.insert(id.to_string(), consumer);
                    }
                } else {
                    self.consumers.remove(id);
                }
            }
            "ssl" => {
                if let Some(val) = value {
                    if let Ok(cert) = serde_json::from_str::<SslCert>(val) {
                        self.ssl_certs.insert(id.to_string(), cert);
                    }
                } else {
                    self.ssl_certs.remove(id);
                }
            }
            "plugin_configs" => {
                if let Some(val) = value {
                    if let Ok(config) = serde_json::from_str::<PluginConfig>(val) {
                        self.plugin_configs.insert(id.to_string(), config);
                    }
                } else {
                    self.plugin_configs.remove(id);
                }
            }
            _ => {
                tracing::warn!(resource_type = resource_type, "Unknown resource type in cache");
            }
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            routes: self.routes.len(),
            services: self.services.len(),
            upstreams: self.upstreams.len(),
            consumers: self.consumers.len(),
            ssl_certs: self.ssl_certs.len(),
            plugin_configs: self.plugin_configs.len(),
        }
    }
}

impl Default for ConfigCache {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub routes: usize,
    pub services: usize,
    pub upstreams: usize,
    pub consumers: usize,
    pub ssl_certs: usize,
    pub plugin_configs: usize,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "routes={}, services={}, upstreams={}, consumers={}, ssl={}, plugin_configs={}",
            self.routes, self.services, self.upstreams, self.consumers, self.ssl_certs, self.plugin_configs
        )
    }
}
