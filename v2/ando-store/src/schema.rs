/// etcd key schema (APISIX-compatible).
pub struct Schema {
    prefix: String,
}

impl Schema {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.trim_end_matches('/').to_string(),
        }
    }

    pub fn routes_prefix(&self) -> String {
        format!("{}/routes/", self.prefix)
    }

    pub fn route_key(&self, id: &str) -> String {
        format!("{}/routes/{}", self.prefix, id)
    }

    pub fn services_prefix(&self) -> String {
        format!("{}/services/", self.prefix)
    }

    pub fn service_key(&self, id: &str) -> String {
        format!("{}/services/{}", self.prefix, id)
    }

    pub fn upstreams_prefix(&self) -> String {
        format!("{}/upstreams/", self.prefix)
    }

    pub fn upstream_key(&self, id: &str) -> String {
        format!("{}/upstreams/{}", self.prefix, id)
    }

    pub fn consumers_prefix(&self) -> String {
        format!("{}/consumers/", self.prefix)
    }

    pub fn consumer_key(&self, id: &str) -> String {
        format!("{}/consumers/{}", self.prefix, id)
    }

    pub fn ssl_prefix(&self) -> String {
        format!("{}/ssl/", self.prefix)
    }

    pub fn ssl_key(&self, id: &str) -> String {
        format!("{}/ssl/{}", self.prefix, id)
    }

    pub fn plugin_configs_prefix(&self) -> String {
        format!("{}/plugin_configs/", self.prefix)
    }

    pub fn plugin_config_key(&self, id: &str) -> String {
        format!("{}/plugin_configs/{}", self.prefix, id)
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self::new("/ando")
    }
}
