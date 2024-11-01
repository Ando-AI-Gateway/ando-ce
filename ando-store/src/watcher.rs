use crate::cache::ConfigCache;
use crate::etcd::EtcdStore;
use crate::schema::KeySchema;
use ando_core::config::EtcdConfig;
use etcd_client::{EventType, WatchOptions};
use tracing::{error, info, warn};

/// Watches etcd for configuration changes and updates the local cache.
pub struct ConfigWatcher {
    config: EtcdConfig,
    cache: ConfigCache,
}

impl ConfigWatcher {
    pub fn new(config: EtcdConfig, cache: ConfigCache) -> Self {
        Self { config, cache }
    }

    /// Perform initial full sync from etcd into the cache.
    pub async fn initial_sync(&self) -> anyhow::Result<()> {
        info!("Starting initial config sync from etcd");

        let mut store = EtcdStore::connect(&self.config).await?;

        for resource_type in &[
            "routes",
            "services",
            "upstreams",
            "consumers",
            "ssl",
            "plugin_configs",
        ] {
            match store.list(resource_type).await {
                Ok(items) => {
                    let schema = KeySchema::new(&self.config.prefix);
                    for (key, value) in &items {
                        if let Some((_, id)) = schema.parse_key(key) {
                            self.cache.apply_change(resource_type, &id, Some(value));
                        }
                    }
                    info!(resource_type = resource_type, count = items.len(), "Synced");
                }
                Err(e) => {
                    error!(resource_type = resource_type, error = %e, "Failed to sync");
                }
            }
        }

        let stats = self.cache.stats();
        info!(%stats, "Initial sync complete");
        Ok(())
    }

    /// Start watching etcd for changes. This runs indefinitely.
    pub async fn watch_forever(&self) -> anyhow::Result<()> {
        let schema = KeySchema::new(&self.config.prefix);
        let prefix = schema.all_prefix();

        info!(prefix = %prefix, "Starting etcd watch");

        let mut client = etcd_client::Client::connect(&self.config.endpoints, None).await?;

        let opts = WatchOptions::new().with_prefix();
        let (_watcher, mut stream) = client.watch(prefix.as_bytes(), Some(opts)).await?;

        info!("etcd watch established");

        while let Some(resp) = stream.message().await? {
            for event in resp.events() {
                let kv = match event.kv() {
                    Some(kv) => kv,
                    None => continue,
                };

                let key = match kv.key_str() {
                    Ok(k) => k.to_string(),
                    Err(_) => continue,
                };

                let (resource_type, id) = match schema.parse_key(&key) {
                    Some(parsed) => parsed,
                    None => {
                        warn!(key = %key, "Unrecognized etcd key");
                        continue;
                    }
                };

                match event.event_type() {
                    EventType::Put => {
                        let value = kv.value_str().unwrap_or("").to_string();
                        info!(
                            resource = %resource_type,
                            id = %id,
                            "Config change detected (PUT)"
                        );
                        self.cache.apply_change(&resource_type, &id, Some(&value));
                    }
                    EventType::Delete => {
                        info!(
                            resource = %resource_type,
                            id = %id,
                            "Config change detected (DELETE)"
                        );
                        self.cache.apply_change(&resource_type, &id, None);
                    }
                }
            }
        }

        warn!("etcd watch stream ended");
        Ok(())
    }
}
