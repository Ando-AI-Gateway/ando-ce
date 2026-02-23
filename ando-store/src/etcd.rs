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
            .get(prefix.as_bytes(), Some(etcd_client::GetOptions::new().with_prefix()))
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
            .get(prefix.as_bytes(), Some(etcd_client::GetOptions::new().with_prefix()))
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
            .get(prefix.as_bytes(), Some(etcd_client::GetOptions::new().with_prefix()))
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
            .get(prefix.as_bytes(), Some(etcd_client::GetOptions::new().with_prefix()))
            .await?;
        for kv in resp.kvs() {
            if let Ok(consumer) =
                serde_json::from_slice::<ando_core::consumer::Consumer>(kv.value())
            {
                cache
                    .consumers
                    .insert(consumer.username.clone(), consumer);
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
