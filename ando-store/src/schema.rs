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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Constructor ──────────────────────────────────────────────

    #[test]
    fn default_prefix_is_ando() {
        let s = Schema::default();
        assert_eq!(s.routes_prefix(), "/ando/routes/");
    }

    #[test]
    fn new_strips_trailing_slash() {
        let s = Schema::new("/ando/");
        assert_eq!(s.routes_prefix(), "/ando/routes/");
    }

    #[test]
    fn new_with_custom_prefix() {
        let s = Schema::new("/myapp");
        assert_eq!(s.routes_prefix(), "/myapp/routes/");
    }

    #[test]
    fn new_with_double_trailing_slash_is_stripped_once() {
        // trim_end_matches strips the trailing char as long as it matches
        let s = Schema::new("/ando//");
        assert_eq!(s.routes_prefix(), "/ando/routes/");
    }

    // ── Routes ───────────────────────────────────────────────────

    #[test]
    fn routes_prefix() {
        let s = Schema::default();
        assert_eq!(s.routes_prefix(), "/ando/routes/");
    }

    #[test]
    fn route_key() {
        let s = Schema::default();
        assert_eq!(s.route_key("abc123"), "/ando/routes/abc123");
    }

    // ── Services ─────────────────────────────────────────────────

    #[test]
    fn services_prefix() {
        assert_eq!(Schema::default().services_prefix(), "/ando/services/");
    }

    #[test]
    fn service_key() {
        assert_eq!(Schema::default().service_key("s1"), "/ando/services/s1");
    }

    // ── Upstreams ────────────────────────────────────────────────

    #[test]
    fn upstreams_prefix() {
        assert_eq!(Schema::default().upstreams_prefix(), "/ando/upstreams/");
    }

    #[test]
    fn upstream_key() {
        assert_eq!(Schema::default().upstream_key("u1"), "/ando/upstreams/u1");
    }

    // ── Consumers ────────────────────────────────────────────────

    #[test]
    fn consumers_prefix() {
        assert_eq!(Schema::default().consumers_prefix(), "/ando/consumers/");
    }

    #[test]
    fn consumer_key() {
        assert_eq!(
            Schema::default().consumer_key("alice"),
            "/ando/consumers/alice"
        );
    }

    // ── SSL ──────────────────────────────────────────────────────

    #[test]
    fn ssl_prefix() {
        assert_eq!(Schema::default().ssl_prefix(), "/ando/ssl/");
    }

    #[test]
    fn ssl_key() {
        assert_eq!(Schema::default().ssl_key("cert-1"), "/ando/ssl/cert-1");
    }

    // ── Plugin configs ───────────────────────────────────────────

    #[test]
    fn plugin_configs_prefix() {
        assert_eq!(
            Schema::default().plugin_configs_prefix(),
            "/ando/plugin_configs/"
        );
    }

    #[test]
    fn plugin_config_key() {
        assert_eq!(
            Schema::default().plugin_config_key("pc1"),
            "/ando/plugin_configs/pc1"
        );
    }

    // ── Key uniqueness ───────────────────────────────────────────

    #[test]
    fn keys_for_same_id_across_namespaces_are_different() {
        let s = Schema::default();
        let id = "shared-id";
        let keys: Vec<String> = vec![
            s.route_key(id),
            s.upstream_key(id),
            s.consumer_key(id),
            s.service_key(id),
            s.ssl_key(id),
            s.plugin_config_key(id),
        ];
        // All keys must be unique
        let mut unique = keys.clone();
        unique.dedup();
        assert_eq!(
            keys.len(),
            unique.len(),
            "All namespace keys must be unique"
        );
    }
}
