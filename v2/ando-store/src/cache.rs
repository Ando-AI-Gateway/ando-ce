use ando_core::consumer::Consumer;
use ando_core::plugin_config::PluginConfig;
use ando_core::route::Route;
use ando_core::service::Service;
use ando_core::ssl::SslCertificate;
use ando_core::upstream::Upstream;
use dashmap::DashMap;
use std::sync::Arc;

/// In-memory config cache — the single source of truth for the data plane.
///
/// v2 design: The DashMap is only accessed from the admin API thread (writes)
/// and during config sync (reads that produce a frozen snapshot). Worker
/// cores never touch this directly — they receive immutable snapshots
/// via crossbeam SPSC channels.
#[derive(Clone)]
pub struct ConfigCache {
    pub routes: Arc<DashMap<String, Route>>,
    pub services: Arc<DashMap<String, Service>>,
    pub upstreams: Arc<DashMap<String, Upstream>>,
    pub consumers: Arc<DashMap<String, Consumer>>,
    pub ssl_certs: Arc<DashMap<String, SslCertificate>>,
    pub plugin_configs: Arc<DashMap<String, PluginConfig>>,
    /// Consumer key → username index (for key-auth O(1) lookup).
    pub consumer_key_index: Arc<DashMap<String, String>>,
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
            consumer_key_index: Arc::new(DashMap::new()),
        }
    }

    /// Rebuild the consumer key index from all consumers.
    pub fn rebuild_consumer_key_index(&self) {
        self.consumer_key_index.clear();
        for entry in self.consumers.iter() {
            let consumer = entry.value();
            if let Some(key_auth_config) = consumer.plugins.get("key-auth") {
                if let Some(key) = key_auth_config.get("key").and_then(|v| v.as_str()) {
                    self.consumer_key_index
                        .insert(key.to_string(), consumer.username.clone());
                }
            }
        }
    }

    /// Look up a consumer by API key (O(1)).
    pub fn find_consumer_by_key(&self, key: &str) -> Option<String> {
        self.consumer_key_index.get(key).map(|v| v.value().clone())
    }

    /// Get all routes as a Vec (for router building).
    pub fn all_routes(&self) -> Vec<Route> {
        self.routes.iter().map(|r| r.value().clone()).collect()
    }
}

impl Default for ConfigCache {
    fn default() -> Self {
        Self::new()
    }
}
