use crate::cache::ConfigCache;
use crate::schema::Schema;
use tracing::info;

/// etcd watcher — watches for config changes and updates the cache.
///
/// v2 design: The watcher runs on a dedicated tokio thread (not monoio).
/// When a change is detected, it updates the DashMap cache and sends a
/// "config changed" signal to all worker cores via crossbeam channels.
pub struct ConfigWatcher {
    schema: Schema,
}

impl ConfigWatcher {
    pub fn new(prefix: &str) -> Self {
        Self {
            schema: Schema::new(prefix),
        }
    }

    /// Start watching etcd for changes. Blocks forever.
    pub async fn watch(
        &self,
        endpoints: &[String],
        cache: ConfigCache,
        notify: crossbeam_channel::Sender<()>,
    ) -> anyhow::Result<()> {
        let mut client = etcd_client::Client::connect(endpoints, None).await?;
        let prefix = format!("{}/", self.schema.routes_prefix().trim_end_matches('/').rsplit('/').skip(1).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/"));

        info!(prefix = %prefix, "Starting etcd watcher");

        let (_watcher, mut stream) = client
            .watch(
                prefix.as_bytes(),
                Some(etcd_client::WatchOptions::new().with_prefix()),
            )
            .await?;

        while let Ok(Ok(Some(resp))) = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            stream.message(),
        )
        .await
        {
            for event in resp.events() {
                if let Some(kv) = event.kv() {
                    let key = String::from_utf8_lossy(kv.key());
                    match event.event_type() {
                        etcd_client::EventType::Put => {
                            self.handle_put(&key, kv.value(), &cache);
                        }
                        etcd_client::EventType::Delete => {
                            self.handle_delete(&key, &cache);
                        }
                    }
                }
            }
            // Notify worker cores that config has changed
            let _ = notify.try_send(());
        }

        Ok(())
    }

    fn handle_put(&self, key: &str, value: &[u8], cache: &ConfigCache) {
        if key.contains("/routes/") {
            if let Ok(route) = serde_json::from_slice::<ando_core::route::Route>(value) {
                info!(route_id = %route.id, "Route updated");
                cache.routes.insert(route.id.clone(), route);
            }
        } else if key.contains("/services/") {
            if let Ok(svc) = serde_json::from_slice::<ando_core::service::Service>(value) {
                cache.services.insert(svc.id.clone(), svc);
            }
        } else if key.contains("/upstreams/") {
            if let Ok(ups) = serde_json::from_slice::<ando_core::upstream::Upstream>(value) {
                if let Some(ref id) = ups.id {
                    cache.upstreams.insert(id.clone(), ups);
                }
            }
        } else if key.contains("/consumers/") {
            if let Ok(consumer) = serde_json::from_slice::<ando_core::consumer::Consumer>(value) {
                cache.consumers.insert(consumer.username.clone(), consumer);
                cache.rebuild_consumer_key_index();
            }
        }
    }

    fn handle_delete(&self, key: &str, cache: &ConfigCache) {
        // Extract ID from key (last path segment)
        let id = key.rsplit('/').next().unwrap_or("");
        if key.contains("/routes/") {
            cache.routes.remove(id);
        } else if key.contains("/services/") {
            cache.services.remove(id);
        } else if key.contains("/upstreams/") {
            cache.upstreams.remove(id);
        } else if key.contains("/consumers/") {
            cache.consumers.remove(id);
            cache.rebuild_consumer_key_index();
        }
    }
}
