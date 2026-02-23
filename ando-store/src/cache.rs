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

#[cfg(test)]
mod tests {
    use super::*;
    use ando_core::consumer::Consumer;
    use ando_core::route::Route;
    use std::collections::HashMap;

    fn make_consumer(username: &str, key: &str) -> Consumer {
        let mut plugins = HashMap::new();
        plugins.insert("key-auth".to_string(), serde_json::json!({ "key": key }));
        Consumer {
            username: username.to_string(),
            plugins,
            desc: None,
            labels: HashMap::new(),
        }
    }

    fn make_route(id: &str, uri: &str) -> Route {
        serde_json::from_value(serde_json::json!({ "id": id, "uri": uri })).unwrap()
    }

    // ── find_consumer_by_key ─────────────────────────────────────

    #[test]
    fn find_consumer_by_key_returns_username() {
        let cache = ConfigCache::new();
        cache.consumers.insert("alice".to_string(), make_consumer("alice", "secret-abc"));
        cache.rebuild_consumer_key_index();
        assert_eq!(cache.find_consumer_by_key("secret-abc"), Some("alice".to_string()));
    }

    #[test]
    fn find_consumer_by_key_unknown_returns_none() {
        let cache = ConfigCache::new();
        cache.rebuild_consumer_key_index();
        assert!(cache.find_consumer_by_key("not-a-key").is_none());
    }

    #[test]
    fn find_consumer_by_key_before_rebuild_returns_none() {
        let cache = ConfigCache::new();
        cache.consumers.insert("bob".to_string(), make_consumer("bob", "bob-key"));
        // Index not rebuilt yet — lookup must return None
        assert!(cache.find_consumer_by_key("bob-key").is_none());
    }

    #[test]
    fn rebuild_consumer_key_index_replaces_stale_entries() {
        let cache = ConfigCache::new();
        cache.consumers.insert("alice".to_string(), make_consumer("alice", "old-key"));
        cache.rebuild_consumer_key_index();

        cache.consumers.remove("alice");
        cache.consumers.insert("alice".to_string(), make_consumer("alice", "new-key"));
        cache.rebuild_consumer_key_index();

        assert!(cache.find_consumer_by_key("old-key").is_none(), "stale key must be removed");
        assert_eq!(cache.find_consumer_by_key("new-key"), Some("alice".to_string()));
    }

    #[test]
    fn rebuild_consumer_key_index_multiple_consumers() {
        let cache = ConfigCache::new();
        cache.consumers.insert("alice".to_string(), make_consumer("alice", "key-a"));
        cache.consumers.insert("bob".to_string(), make_consumer("bob", "key-b"));
        cache.consumers.insert("carol".to_string(), make_consumer("carol", "key-c"));
        cache.rebuild_consumer_key_index();

        assert_eq!(cache.find_consumer_by_key("key-a"), Some("alice".to_string()));
        assert_eq!(cache.find_consumer_by_key("key-b"), Some("bob".to_string()));
        assert_eq!(cache.find_consumer_by_key("key-c"), Some("carol".to_string()));
    }

    #[test]
    fn consumer_without_key_auth_plugin_not_indexed() {
        let cache = ConfigCache::new();
        let consumer = Consumer {
            username: "noauth".to_string(),
            plugins: HashMap::new(),
            desc: None,
            labels: HashMap::new(),
        };
        cache.consumers.insert("noauth".to_string(), consumer);
        cache.rebuild_consumer_key_index();
        assert_eq!(cache.consumer_key_index.len(), 0);
    }

    // ── all_routes ───────────────────────────────────────────────

    #[test]
    fn all_routes_empty_when_no_routes() {
        let cache = ConfigCache::new();
        assert!(cache.all_routes().is_empty());
    }

    #[test]
    fn all_routes_returns_all_inserted() {
        let cache = ConfigCache::new();
        cache.routes.insert("r1".to_string(), make_route("r1", "/a"));
        cache.routes.insert("r2".to_string(), make_route("r2", "/b"));
        cache.routes.insert("r3".to_string(), make_route("r3", "/c"));
        assert_eq!(cache.all_routes().len(), 3);
    }

    #[test]
    fn all_routes_reflects_removal() {
        let cache = ConfigCache::new();
        cache.routes.insert("r1".to_string(), make_route("r1", "/a"));
        cache.routes.insert("r2".to_string(), make_route("r2", "/b"));
        cache.routes.remove("r1");
        let routes = cache.all_routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].id, "r2");
    }

    // ── clone shares underlying DashMaps ────────────────────────

    #[test]
    fn clone_shares_dashmap_state() {
        let cache = ConfigCache::new();
        let clone = cache.clone();
        cache.routes.insert("rx".to_string(), make_route("rx", "/shared"));
        // clone holds Arc to same DashMap
        assert_eq!(clone.routes.len(), 1);
    }

    // ── default ─────────────────────────────────────────────────

    #[test]
    fn default_is_empty() {
        let cache = ConfigCache::default();
        assert!(cache.routes.is_empty());
        assert!(cache.consumers.is_empty());
        assert!(cache.upstreams.is_empty());
        assert!(cache.consumer_key_index.is_empty());
    }
}
