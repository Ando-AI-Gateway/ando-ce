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
        let prefix = format!(
            "{}/",
            self.schema
                .routes_prefix()
                .trim_end_matches('/')
                .rsplit('/')
                .skip(1)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("/")
        );

        info!(prefix = %prefix, "Starting etcd watcher");

        let (_watcher, mut stream) = client
            .watch(
                prefix.as_bytes(),
                Some(etcd_client::WatchOptions::new().with_prefix()),
            )
            .await?;

        while let Ok(Ok(Some(resp))) =
            tokio::time::timeout(std::time::Duration::from_secs(30), stream.message()).await
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
            if let Ok(ups) = serde_json::from_slice::<ando_core::upstream::Upstream>(value)
                && let Some(ref id) = ups.id
            {
                cache.upstreams.insert(id.clone(), ups);
            }
        } else if key.contains("/consumers/")
            && let Ok(consumer) = serde_json::from_slice::<ando_core::consumer::Consumer>(value)
        {
            cache.consumers.insert(consumer.username.clone(), consumer);
            cache.rebuild_consumer_key_index();
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

#[cfg(test)]
mod tests {
    use super::*;
    use ando_core::consumer::Consumer;
    use ando_core::route::Route;
    use ando_core::service::Service;
    use ando_core::upstream::Upstream;
    use std::collections::HashMap;

    fn watcher() -> ConfigWatcher {
        ConfigWatcher::new("/ando")
    }

    fn make_route(id: &str) -> Route {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "uri": format!("/test/{id}"),
        }))
        .unwrap()
    }

    fn make_service(id: &str) -> Service {
        serde_json::from_value(serde_json::json!({ "id": id })).unwrap()
    }

    fn make_upstream(id: &str) -> Upstream {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "nodes": { "127.0.0.1:8080": 1 },
        }))
        .unwrap()
    }

    fn make_consumer(username: &str, api_key: Option<&str>) -> Consumer {
        let mut plugins = HashMap::new();
        if let Some(key) = api_key {
            plugins.insert("key-auth".to_string(), serde_json::json!({ "key": key }));
        }
        Consumer {
            username: username.to_string(),
            plugins,
            desc: None,
            labels: HashMap::new(),
        }
    }

    // ── handle_put: routes ──────────────────────────────────────

    #[test]
    fn handle_put_inserts_route() {
        let w = watcher();
        let cache = ConfigCache::new();
        let route = make_route("r1");
        let data = serde_json::to_vec(&route).unwrap();
        w.handle_put("/ando/routes/r1", &data, &cache);
        assert_eq!(cache.routes.len(), 1);
        assert_eq!(cache.routes.get("r1").unwrap().uri, "/test/r1");
    }

    #[test]
    fn handle_put_updates_existing_route() {
        let w = watcher();
        let cache = ConfigCache::new();
        let route1 = make_route("r1");
        w.handle_put(
            "/ando/routes/r1",
            &serde_json::to_vec(&route1).unwrap(),
            &cache,
        );

        let route2: Route = serde_json::from_value(serde_json::json!({
            "id": "r1", "uri": "/updated"
        }))
        .unwrap();
        w.handle_put(
            "/ando/routes/r1",
            &serde_json::to_vec(&route2).unwrap(),
            &cache,
        );
        assert_eq!(cache.routes.len(), 1);
        assert_eq!(cache.routes.get("r1").unwrap().uri, "/updated");
    }

    // ── handle_put: services ────────────────────────────────────

    #[test]
    fn handle_put_inserts_service() {
        let w = watcher();
        let cache = ConfigCache::new();
        let svc = make_service("svc1");
        w.handle_put(
            "/ando/services/svc1",
            &serde_json::to_vec(&svc).unwrap(),
            &cache,
        );
        assert_eq!(cache.services.len(), 1);
        assert!(cache.services.get("svc1").is_some());
    }

    // ── handle_put: upstreams ───────────────────────────────────

    #[test]
    fn handle_put_inserts_upstream() {
        let w = watcher();
        let cache = ConfigCache::new();
        let ups = make_upstream("ups1");
        w.handle_put(
            "/ando/upstreams/ups1",
            &serde_json::to_vec(&ups).unwrap(),
            &cache,
        );
        assert_eq!(cache.upstreams.len(), 1);
        assert!(cache.upstreams.get("ups1").is_some());
    }

    #[test]
    fn handle_put_upstream_without_id_is_ignored() {
        let w = watcher();
        let cache = ConfigCache::new();
        // Upstream with no id field
        let data = serde_json::to_vec(&serde_json::json!({
            "nodes": { "127.0.0.1:8080": 1 },
        }))
        .unwrap();
        w.handle_put("/ando/upstreams/ups1", &data, &cache);
        assert_eq!(cache.upstreams.len(), 0);
    }

    // ── handle_put: consumers ───────────────────────────────────

    #[test]
    fn handle_put_inserts_consumer_and_rebuilds_key_index() {
        let w = watcher();
        let cache = ConfigCache::new();
        let consumer = make_consumer("alice", Some("secret-abc"));
        w.handle_put(
            "/ando/consumers/alice",
            &serde_json::to_vec(&consumer).unwrap(),
            &cache,
        );
        assert_eq!(cache.consumers.len(), 1);
        assert_eq!(
            cache.find_consumer_by_key("secret-abc"),
            Some("alice".to_string())
        );
    }

    #[test]
    fn handle_put_consumer_without_key_auth_rebuilds_index() {
        let w = watcher();
        let cache = ConfigCache::new();
        let consumer = make_consumer("bob", None);
        w.handle_put(
            "/ando/consumers/bob",
            &serde_json::to_vec(&consumer).unwrap(),
            &cache,
        );
        assert_eq!(cache.consumers.len(), 1);
        assert!(cache.consumer_key_index.is_empty());
    }

    // ── handle_put: invalid JSON ────────────────────────────────

    #[test]
    fn handle_put_with_invalid_json_is_silently_ignored() {
        let w = watcher();
        let cache = ConfigCache::new();
        w.handle_put("/ando/routes/r1", b"not-json", &cache);
        assert_eq!(cache.routes.len(), 0);
    }

    #[test]
    fn handle_put_with_wrong_schema_json_is_ignored() {
        let w = watcher();
        let cache = ConfigCache::new();
        // Valid JSON but missing required "id" field for Route
        w.handle_put("/ando/routes/r1", br#"{"foo":"bar"}"#, &cache);
        assert_eq!(cache.routes.len(), 0);
    }

    // ── handle_put: unknown key prefix ──────────────────────────

    #[test]
    fn handle_put_unknown_prefix_is_noop() {
        let w = watcher();
        let cache = ConfigCache::new();
        w.handle_put("/ando/ssl/cert1", br#"{"id":"cert1"}"#, &cache);
        assert_eq!(cache.routes.len(), 0);
        assert_eq!(cache.services.len(), 0);
        assert_eq!(cache.upstreams.len(), 0);
        assert_eq!(cache.consumers.len(), 0);
    }

    // ── handle_delete: routes ───────────────────────────────────

    #[test]
    fn handle_delete_removes_route() {
        let w = watcher();
        let cache = ConfigCache::new();
        let route = make_route("r1");
        cache.routes.insert("r1".to_string(), route);
        assert_eq!(cache.routes.len(), 1);

        w.handle_delete("/ando/routes/r1", &cache);
        assert_eq!(cache.routes.len(), 0);
    }

    // ── handle_delete: services ─────────────────────────────────

    #[test]
    fn handle_delete_removes_service() {
        let w = watcher();
        let cache = ConfigCache::new();
        let svc = make_service("svc1");
        cache.services.insert("svc1".to_string(), svc);

        w.handle_delete("/ando/services/svc1", &cache);
        assert_eq!(cache.services.len(), 0);
    }

    // ── handle_delete: upstreams ────────────────────────────────

    #[test]
    fn handle_delete_removes_upstream() {
        let w = watcher();
        let cache = ConfigCache::new();
        let ups = make_upstream("ups1");
        cache.upstreams.insert("ups1".to_string(), ups);

        w.handle_delete("/ando/upstreams/ups1", &cache);
        assert_eq!(cache.upstreams.len(), 0);
    }

    // ── handle_delete: consumers ────────────────────────────────

    #[test]
    fn handle_delete_removes_consumer_and_rebuilds_key_index() {
        let w = watcher();
        let cache = ConfigCache::new();
        let consumer = make_consumer("alice", Some("secret-abc"));
        cache.consumers.insert("alice".to_string(), consumer);
        cache.rebuild_consumer_key_index();
        assert!(cache.find_consumer_by_key("secret-abc").is_some());

        w.handle_delete("/ando/consumers/alice", &cache);
        assert_eq!(cache.consumers.len(), 0);
        assert!(cache.find_consumer_by_key("secret-abc").is_none());
    }

    // ── handle_delete: non-existent key ─────────────────────────

    #[test]
    fn handle_delete_nonexistent_key_is_noop() {
        let w = watcher();
        let cache = ConfigCache::new();
        w.handle_delete("/ando/routes/does-not-exist", &cache);
        assert_eq!(cache.routes.len(), 0);
    }

    // ── handle_delete: unknown prefix ───────────────────────────

    #[test]
    fn handle_delete_unknown_prefix_is_noop() {
        let w = watcher();
        let cache = ConfigCache::new();
        let route = make_route("r1");
        cache.routes.insert("r1".to_string(), route);

        w.handle_delete("/ando/ssl/cert1", &cache);
        assert_eq!(cache.routes.len(), 1); // unchanged
    }

    // ── multiple entities ───────────────────────────────────────

    #[test]
    fn handle_put_multiple_entity_types() {
        let w = watcher();
        let cache = ConfigCache::new();

        let route = make_route("r1");
        let svc = make_service("svc1");
        let ups = make_upstream("ups1");
        let consumer = make_consumer("bob", Some("key-bob"));

        w.handle_put(
            "/ando/routes/r1",
            &serde_json::to_vec(&route).unwrap(),
            &cache,
        );
        w.handle_put(
            "/ando/services/svc1",
            &serde_json::to_vec(&svc).unwrap(),
            &cache,
        );
        w.handle_put(
            "/ando/upstreams/ups1",
            &serde_json::to_vec(&ups).unwrap(),
            &cache,
        );
        w.handle_put(
            "/ando/consumers/bob",
            &serde_json::to_vec(&consumer).unwrap(),
            &cache,
        );

        assert_eq!(cache.routes.len(), 1);
        assert_eq!(cache.services.len(), 1);
        assert_eq!(cache.upstreams.len(), 1);
        assert_eq!(cache.consumers.len(), 1);
        assert_eq!(
            cache.find_consumer_by_key("key-bob"),
            Some("bob".to_string())
        );
    }
}
