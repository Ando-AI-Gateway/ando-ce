use crate::schema::KeySchema;
use ando_core::config::EtcdConfig;
use etcd_client::{Client, GetOptions};
use tracing::{debug, info};

/// Wrapper around the etcd v3 client with Ando-specific operations.
pub struct EtcdStore {
    client: Client,
    schema: KeySchema,
}

impl EtcdStore {
    /// Connect to etcd.
    pub async fn connect(config: &EtcdConfig) -> anyhow::Result<Self> {
        info!(endpoints = ?config.endpoints, prefix = %config.prefix, "Connecting to etcd");

        let client = Client::connect(&config.endpoints, None).await?;

        // Note: For etcd authentication, use ConnectOptions when connecting.
        // let opts = ConnectOptions::new().with_user(user, pass);
        // let client = Client::connect(&config.endpoints, Some(opts)).await?;

        let schema = KeySchema::new(&config.prefix);

        Ok(Self { client, schema })
    }

    /// Get a value by resource type and ID.
    pub async fn get(&mut self, resource_type: &str, id: &str) -> anyhow::Result<Option<String>> {
        let key = match resource_type {
            "routes" => self.schema.route_key(id),
            "services" => self.schema.service_key(id),
            "upstreams" => self.schema.upstream_key(id),
            "consumers" => self.schema.consumer_key(id),
            "ssl" => self.schema.ssl_key(id),
            "plugin_configs" => self.schema.plugin_config_key(id),
            _ => anyhow::bail!("Unknown resource type: {}", resource_type),
        };

        let resp = self.client.get(key.as_bytes(), None).await?;
        if let Some(kv) = resp.kvs().first() {
            Ok(Some(kv.value_str()?.to_string()))
        } else {
            Ok(None)
        }
    }

    /// List all values for a resource type.
    pub async fn list(&mut self, resource_type: &str) -> anyhow::Result<Vec<(String, String)>> {
        let prefix = match resource_type {
            "routes" => self.schema.routes_prefix(),
            "services" => self.schema.services_prefix(),
            "upstreams" => self.schema.upstreams_prefix(),
            "consumers" => self.schema.consumers_prefix(),
            "ssl" => self.schema.ssl_prefix(),
            "plugin_configs" => self.schema.plugin_configs_prefix(),
            _ => anyhow::bail!("Unknown resource type: {}", resource_type),
        };

        let opts = GetOptions::new().with_prefix();
        let resp = self.client.get(prefix.as_bytes(), Some(opts)).await?;

        let mut results = Vec::new();
        for kv in resp.kvs() {
            let key = kv.key_str()?.to_string();
            let value = kv.value_str()?.to_string();
            results.push((key, value));
        }

        debug!(resource_type = resource_type, count = results.len(), "Listed resources");
        Ok(results)
    }

    /// Put a value.
    pub async fn put(
        &mut self,
        resource_type: &str,
        id: &str,
        value: &str,
    ) -> anyhow::Result<()> {
        let key = match resource_type {
            "routes" => self.schema.route_key(id),
            "services" => self.schema.service_key(id),
            "upstreams" => self.schema.upstream_key(id),
            "consumers" => self.schema.consumer_key(id),
            "ssl" => self.schema.ssl_key(id),
            "plugin_configs" => self.schema.plugin_config_key(id),
            _ => anyhow::bail!("Unknown resource type: {}", resource_type),
        };

        self.client.put(key.as_bytes(), value.as_bytes(), None).await?;
        debug!(resource_type = resource_type, id = id, "Put resource");
        Ok(())
    }

    /// Delete a value.
    pub async fn delete(&mut self, resource_type: &str, id: &str) -> anyhow::Result<bool> {
        let key = match resource_type {
            "routes" => self.schema.route_key(id),
            "services" => self.schema.service_key(id),
            "upstreams" => self.schema.upstream_key(id),
            "consumers" => self.schema.consumer_key(id),
            "ssl" => self.schema.ssl_key(id),
            "plugin_configs" => self.schema.plugin_config_key(id),
            _ => anyhow::bail!("Unknown resource type: {}", resource_type),
        };

        let resp = self.client.delete(key.as_bytes(), None).await?;
        let deleted = resp.deleted() > 0;
        debug!(resource_type = resource_type, id = id, deleted = deleted, "Delete resource");
        Ok(deleted)
    }

    /// Get the underlying schema.
    pub fn schema(&self) -> &KeySchema {
        &self.schema
    }

    /// Get a mutable reference to the client (for watchers).
    pub fn client_mut(&mut self) -> &mut Client {
        &mut self.client
    }
}
