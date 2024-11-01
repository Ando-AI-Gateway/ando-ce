/// etcd key schema for Ando.
///
/// All keys are prefixed with the configured prefix (default: `/ando`).
///
/// Schema:
/// ```text
/// /ando/routes/{route_id}
/// /ando/services/{service_id}
/// /ando/upstreams/{upstream_id}
/// /ando/consumers/{consumer_id}
/// /ando/ssl/{ssl_id}
/// /ando/plugin_configs/{config_id}
/// /ando/global_rules/{rule_id}
/// ```
pub struct KeySchema {
    prefix: String,
}

impl KeySchema {
    pub fn new(prefix: &str) -> Self {
        let prefix = prefix.trim_end_matches('/').to_string();
        Self { prefix }
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

    pub fn global_rules_prefix(&self) -> String {
        format!("{}/global_rules/", self.prefix)
    }

    /// Get the full prefix for watching all config changes.
    pub fn all_prefix(&self) -> String {
        format!("{}/", self.prefix)
    }

    /// Extract the resource type and ID from a key.
    pub fn parse_key(&self, key: &str) -> Option<(String, String)> {
        let suffix = key.strip_prefix(&format!("{}/", self.prefix))?;
        let mut parts = suffix.splitn(2, '/');
        let resource_type = parts.next()?.to_string();
        let id = parts.next()?.to_string();
        Some((resource_type, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_schema() {
        let schema = KeySchema::new("/ando");
        assert_eq!(schema.route_key("r1"), "/ando/routes/r1");
        assert_eq!(schema.upstream_key("u1"), "/ando/upstreams/u1");

        let (rtype, id) = schema.parse_key("/ando/routes/r1").unwrap();
        assert_eq!(rtype, "routes");
        assert_eq!(id, "r1");
    }
}
