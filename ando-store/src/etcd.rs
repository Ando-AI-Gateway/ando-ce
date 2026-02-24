use crate::cache::ConfigCache;
use crate::schema::Schema;
use anyhow::Result;
use tracing::info;

/// etcd client wrapper for CRUD operations.
pub struct EtcdStore {
    client: etcd_client::Client,
    schema: Schema,
}

impl EtcdStore {
    /// Connect to etcd.
    pub async fn connect(endpoints: &[String], prefix: &str) -> Result<Self> {
        let client = etcd_client::Client::connect(endpoints, None).await?;
        info!("Connected to etcd at {:?}", endpoints);
        Ok(Self {
            client,
            schema: Schema::new(prefix),
        })
    }

    /// Load all config from etcd into the cache.
    pub async fn load_all(&mut self, cache: &ConfigCache) -> Result<()> {
        self.load_routes(cache).await?;
        self.load_services(cache).await?;
        self.load_upstreams(cache).await?;
        self.load_consumers(cache).await?;
        cache.rebuild_consumer_key_index();
        info!("Loaded all config from etcd");
        Ok(())
    }

    async fn load_routes(&mut self, cache: &ConfigCache) -> Result<()> {
        let prefix = self.schema.routes_prefix();
        let resp = self
            .client
            .get(
                prefix.as_bytes(),
                Some(etcd_client::GetOptions::new().with_prefix()),
            )
            .await?;
        for kv in resp.kvs() {
            if let Ok(route) = serde_json::from_slice::<ando_core::route::Route>(kv.value()) {
                cache.routes.insert(route.id.clone(), route);
            }
        }
        Ok(())
    }

    async fn load_services(&mut self, cache: &ConfigCache) -> Result<()> {
        let prefix = self.schema.services_prefix();
        let resp = self
            .client
            .get(
                prefix.as_bytes(),
                Some(etcd_client::GetOptions::new().with_prefix()),
            )
            .await?;
        for kv in resp.kvs() {
            if let Ok(svc) = serde_json::from_slice::<ando_core::service::Service>(kv.value()) {
                cache.services.insert(svc.id.clone(), svc);
            }
        }
        Ok(())
    }

    async fn load_upstreams(&mut self, cache: &ConfigCache) -> Result<()> {
        let prefix = self.schema.upstreams_prefix();
        let resp = self
            .client
            .get(
                prefix.as_bytes(),
                Some(etcd_client::GetOptions::new().with_prefix()),
            )
            .await?;
        for kv in resp.kvs() {
            if let Ok(ups) = serde_json::from_slice::<ando_core::upstream::Upstream>(kv.value())
                && let Some(ref id) = ups.id
            {
                cache.upstreams.insert(id.clone(), ups);
            }
        }
        Ok(())
    }

    async fn load_consumers(&mut self, cache: &ConfigCache) -> Result<()> {
        let prefix = self.schema.consumers_prefix();
        let resp = self
            .client
            .get(
                prefix.as_bytes(),
                Some(etcd_client::GetOptions::new().with_prefix()),
            )
            .await?;
        for kv in resp.kvs() {
            if let Ok(consumer) =
                serde_json::from_slice::<ando_core::consumer::Consumer>(kv.value())
            {
                cache.consumers.insert(consumer.username.clone(), consumer);
            }
        }
        Ok(())
    }

    /// Put a route into etcd.
    pub async fn put_route(&mut self, route: &ando_core::route::Route) -> Result<()> {
        let key = self.schema.route_key(&route.id);
        let value = serde_json::to_vec(route)?;
        self.client.put(key, value, None).await?;
        Ok(())
    }

    /// Delete a route from etcd.
    pub async fn delete_route(&mut self, id: &str) -> Result<()> {
        let key = self.schema.route_key(id);
        self.client.delete(key, None).await?;
        Ok(())
    }

    /// Put an upstream into etcd.
    pub async fn put_upstream(&mut self, upstream: &ando_core::upstream::Upstream) -> Result<()> {
        if let Some(ref id) = upstream.id {
            let key = self.schema.upstream_key(id);
            let value = serde_json::to_vec(upstream)?;
            self.client.put(key, value, None).await?;
        }
        Ok(())
    }

    /// Put a consumer into etcd.
    pub async fn put_consumer(&mut self, consumer: &ando_core::consumer::Consumer) -> Result<()> {
        let key = self.schema.consumer_key(&consumer.username);
        let value = serde_json::to_vec(consumer)?;
        self.client.put(key, value, None).await?;
        Ok(())
    }

    /// Put a service into etcd.
    pub async fn put_service(&mut self, service: &ando_core::service::Service) -> Result<()> {
        let key = self.schema.service_key(&service.id);
        let value = serde_json::to_vec(service)?;
        self.client.put(key, value, None).await?;
        Ok(())
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
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

    // ── Serialization roundtrips (the exact payloads EtcdStore sends) ──

    #[test]
    fn route_serde_roundtrip_for_etcd() {
        let route: Route = serde_json::from_value(serde_json::json!({
            "id": "r1",
            "uri": "/api/v1/*",
            "methods": ["GET", "POST"],
            "plugins": { "rate-limiting": { "count": 100 } }
        }))
        .unwrap();
        let bytes = serde_json::to_vec(&route).unwrap();
        let decoded: Route = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.id, "r1");
        assert_eq!(decoded.uri, "/api/v1/*");
        assert_eq!(decoded.methods, vec!["GET", "POST"]);
        assert!(decoded.plugins.contains_key("rate-limiting"));
    }

    #[test]
    fn service_serde_roundtrip_for_etcd() {
        let svc = Service {
            id: "svc1".into(),
            name: Some("my-service".into()),
            desc: None,
            upstream_id: Some("ups1".into()),
            upstream: None,
            plugins: HashMap::new(),
            labels: HashMap::new(),
        };
        let bytes = serde_json::to_vec(&svc).unwrap();
        let decoded: Service = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.id, "svc1");
        assert_eq!(decoded.upstream_id, Some("ups1".into()));
    }

    #[test]
    fn upstream_serde_roundtrip_for_etcd() {
        let ups: Upstream = serde_json::from_value(serde_json::json!({
            "id": "ups1",
            "nodes": { "10.0.0.1:8080": 10, "10.0.0.2:8080": 5 },
            "type": "roundrobin"
        }))
        .unwrap();
        let bytes = serde_json::to_vec(&ups).unwrap();
        let decoded: Upstream = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.id, Some("ups1".into()));
        assert_eq!(decoded.nodes.len(), 2);
        assert_eq!(decoded.lb_type, "roundrobin");
    }

    #[test]
    fn consumer_serde_roundtrip_for_etcd() {
        let mut plugins = HashMap::new();
        plugins.insert(
            "key-auth".to_string(),
            serde_json::json!({ "key": "my-secret" }),
        );
        let consumer = Consumer {
            username: "alice".into(),
            plugins,
            desc: Some("Test consumer".into()),
            labels: HashMap::new(),
        };
        let bytes = serde_json::to_vec(&consumer).unwrap();
        let decoded: Consumer = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.username, "alice");
        assert_eq!(decoded.plugins["key-auth"]["key"], "my-secret");
    }

    // ── Key generation matches etcd paths ───────────────────────

    #[test]
    fn etcd_store_uses_correct_route_key() {
        let schema = Schema::new("/ando");
        let route: Route =
            serde_json::from_value(serde_json::json!({"id": "route-42", "uri": "/"})).unwrap();
        let key = schema.route_key(&route.id);
        assert_eq!(key, "/ando/routes/route-42");
    }

    #[test]
    fn etcd_store_uses_correct_upstream_key() {
        let schema = Schema::new("/ando");
        let ups: Upstream = serde_json::from_value(serde_json::json!({
            "id": "ups-99",
            "nodes": {}
        }))
        .unwrap();
        let key = schema.upstream_key(ups.id.as_ref().unwrap());
        assert_eq!(key, "/ando/upstreams/ups-99");
    }

    #[test]
    fn etcd_store_uses_correct_consumer_key() {
        let schema = Schema::new("/ando");
        let consumer = Consumer {
            username: "bob".into(),
            plugins: HashMap::new(),
            desc: None,
            labels: HashMap::new(),
        };
        let key = schema.consumer_key(&consumer.username);
        assert_eq!(key, "/ando/consumers/bob");
    }

    #[test]
    fn etcd_store_uses_correct_service_key() {
        let schema = Schema::new("/ando");
        let svc: Service = serde_json::from_value(serde_json::json!({"id": "svc-7"})).unwrap();
        let key = schema.service_key(&svc.id);
        assert_eq!(key, "/ando/services/svc-7");
    }

    // ── load_* deserialization (simulates what etcd returns) ─────

    #[test]
    fn load_routes_deserializes_valid_json_bytes() {
        let route: Route =
            serde_json::from_value(serde_json::json!({"id": "r1", "uri": "/test"})).unwrap();
        let bytes = serde_json::to_vec(&route).unwrap();
        let decoded: Result<Route, _> = serde_json::from_slice(&bytes);
        assert!(decoded.is_ok());
        assert_eq!(decoded.unwrap().id, "r1");
    }

    #[test]
    fn load_routes_skips_invalid_json_bytes() {
        let decoded: Result<Route, _> = serde_json::from_slice(b"not-json");
        assert!(decoded.is_err());
    }

    #[test]
    fn load_upstreams_skips_upstream_without_id() {
        let ups: Upstream = serde_json::from_value(serde_json::json!({
            "nodes": { "127.0.0.1:80": 1 }
        }))
        .unwrap();
        // Upstream deserialized but has no id — EtcdStore::load_upstreams would skip it
        assert!(ups.id.is_none());
    }

    // ── cache population (load_all logic without etcd) ──────────

    #[test]
    fn cache_population_from_multiple_entities() {
        let cache = ConfigCache::new();

        // Simulate what load_all does: deserialize from bytes and insert
        let route: Route =
            serde_json::from_value(serde_json::json!({"id": "r1", "uri": "/a"})).unwrap();
        cache.routes.insert(route.id.clone(), route);

        let svc: Service = serde_json::from_value(serde_json::json!({"id": "svc1"})).unwrap();
        cache.services.insert(svc.id.clone(), svc);

        let ups: Upstream = serde_json::from_value(serde_json::json!({
            "id": "ups1", "nodes": {}
        }))
        .unwrap();
        cache.upstreams.insert(ups.id.clone().unwrap(), ups);

        let consumer = Consumer {
            username: "alice".into(),
            plugins: {
                let mut m = HashMap::new();
                m.insert("key-auth".into(), serde_json::json!({"key": "k1"}));
                m
            },
            desc: None,
            labels: HashMap::new(),
        };
        cache.consumers.insert(consumer.username.clone(), consumer);
        cache.rebuild_consumer_key_index();

        assert_eq!(cache.routes.len(), 1);
        assert_eq!(cache.services.len(), 1);
        assert_eq!(cache.upstreams.len(), 1);
        assert_eq!(cache.consumers.len(), 1);
        assert_eq!(cache.find_consumer_by_key("k1"), Some("alice".to_string()));
    }
}
