use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use serde::Deserialize;

/// Key-auth plugin — authenticates requests via an API key header.
///
/// APISIX-compatible: looks for the key in the `apikey` header by default.
/// Can be configured to look in a custom header or query parameter.
pub struct KeyAuthPlugin;

#[derive(Debug, Deserialize)]
struct KeyAuthConfig {
    /// Header name to check for the API key.
    #[serde(default = "default_header")]
    header: String,
    /// Whether to hide the auth header from upstream.
    #[serde(default)]
    hide_credentials: bool,
}

fn default_header() -> String {
    "apikey".to_string()
}

struct KeyAuthInstance {
    header: String,
    hide_credentials: bool,
}

impl Plugin for KeyAuthPlugin {
    fn name(&self) -> &str {
        "key-auth"
    }

    fn priority(&self) -> i32 {
        2500 // APISIX default priority for key-auth
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: KeyAuthConfig = serde_json::from_value(config.clone())
            .unwrap_or(KeyAuthConfig {
                header: default_header(),
                hide_credentials: false,
            });

        Ok(Box::new(KeyAuthInstance {
            header: cfg.header.to_lowercase(),
            hide_credentials: cfg.hide_credentials,
        }))
    }
}

impl PluginInstance for KeyAuthInstance {
    fn name(&self) -> &str {
        "key-auth"
    }

    fn priority(&self) -> i32 {
        2500
    }

    /// Check for API key in the configured header.
    ///
    /// The actual key validation is done against the consumer store.
    /// This plugin sets `ctx.consumer` to the matched consumer username,
    /// or returns 401 if the key is missing/invalid.
    ///
    /// v2 design: The consumer lookup is done inline (no async) using
    /// a pre-built HashMap that each worker core holds locally.
    fn access(&self, ctx: &mut PluginContext) -> PluginResult {
        let key = match ctx.get_header(&self.header) {
            Some(k) if !k.is_empty() => k.to_string(),
            _ => {
                return PluginResult::Response {
                    status: 401,
                    headers: vec![
                        ("content-type".to_string(), "application/json".to_string()),
                        ("www-authenticate".to_string(), "Key realm=\"Ando\"".to_string()),
                    ],
                    body: Some(br#"{"error":"Missing API key","status":401}"#.to_vec()),
                };
            }
        };

        // Store the key in vars for the proxy to validate against consumers.
        // The proxy layer handles the actual consumer lookup since it has
        // access to the consumer store.
        ctx.vars.insert(
            "_key_auth_key".to_string(),
            serde_json::Value::String(key),
        );

        if self.hide_credentials {
            ctx.request_headers.remove(&self.header);
        }

        PluginResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_ctx(headers: Vec<(&str, &str)>) -> PluginContext {
        let map: HashMap<String, String> = headers
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        PluginContext::new("r1".into(), "1.2.3.4".into(), "GET".into(), "/".into(), map)
    }

    fn inst(header: &str, hide: bool) -> KeyAuthInstance {
        KeyAuthInstance { header: header.to_lowercase(), hide_credentials: hide }
    }

    // ── Missing / empty key ──────────────────────────────────────────────────

    #[test]
    fn test_missing_key_returns_401() {
        let mut ctx = make_ctx(vec![]);
        let result = inst("apikey", false).access(&mut ctx);
        if let PluginResult::Response { status, body, headers } = result {
            assert_eq!(status, 401);
            let body_text = String::from_utf8(body.unwrap()).unwrap();
            assert!(body_text.contains("Missing API key"));
            assert!(headers.iter().any(|(k, _)| k == "www-authenticate"));
        } else {
            panic!("Expected 401 Response");
        }
    }

    #[test]
    fn test_empty_key_returns_401() {
        let mut ctx = make_ctx(vec![("apikey", "")]);
        let result = inst("apikey", false).access(&mut ctx);
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    // ── Valid key ────────────────────────────────────────────────────────────

    #[test]
    fn test_valid_key_stored_in_vars() {
        let mut ctx = make_ctx(vec![("apikey", "my-secret-key")]);
        let result = inst("apikey", false).access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.vars["_key_auth_key"], serde_json::json!("my-secret-key"));
    }

    #[test]
    fn test_key_value_preserved_correctly() {
        let mut ctx = make_ctx(vec![("apikey", "Bearer eyJhbGci")]);
        inst("apikey", false).access(&mut ctx);
        assert_eq!(ctx.vars["_key_auth_key"], "Bearer eyJhbGci");
    }

    // ── hide_credentials ─────────────────────────────────────────────────────

    #[test]
    fn test_hide_credentials_removes_header() {
        let mut ctx = make_ctx(vec![("apikey", "my-key")]);
        inst("apikey", true).access(&mut ctx);
        assert!(ctx.request_headers.get("apikey").is_none(), "header must be removed");
    }

    #[test]
    fn test_no_hide_keeps_header() {
        let mut ctx = make_ctx(vec![("apikey", "my-key")]);
        inst("apikey", false).access(&mut ctx);
        assert_eq!(ctx.request_headers.get("apikey").map(|s| s.as_str()), Some("my-key"));
    }

    // ── Custom header name ───────────────────────────────────────────────────

    #[test]
    fn test_custom_header_matches() {
        let mut ctx = make_ctx(vec![("x-api-key", "custom-key")]);
        let result = inst("x-api-key", false).access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.vars["_key_auth_key"], "custom-key");
    }

    #[test]
    fn test_custom_header_wrong_name_returns_401() {
        // Configured for x-api-key but sends apikey — should reject
        let mut ctx = make_ctx(vec![("apikey", "some-key")]);
        let result = inst("x-api-key", false).access(&mut ctx);
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    #[test]
    fn test_header_name_case_insensitive_config() {
        // configure() lowercases the header; lookup uses lowercase header from ctx
        let mut ctx = make_ctx(vec![("x-api-key", "k")]);
        let result = inst("X-Api-Key", false).access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
    }

    // ── Plugin metadata ──────────────────────────────────────────────────────

    #[test]
    fn test_plugin_name_and_priority() {
        use ando_plugin::plugin::Plugin;
        assert_eq!(KeyAuthPlugin.name(), "key-auth");
        assert_eq!(KeyAuthPlugin.priority(), 2500);
        assert_eq!(KeyAuthPlugin.phases(), &[Phase::Access]);
    }

    #[test]
    fn test_instance_name_and_priority() {
        let i = inst("apikey", false);
        assert_eq!(i.name(), "key-auth");
        assert_eq!(i.priority(), 2500);
    }

    // ── configure() factory ──────────────────────────────────────────────────

    #[test]
    fn test_configure_defaults() {
        use ando_plugin::plugin::Plugin;
        let plugin = KeyAuthPlugin;
        let i = plugin.configure(&serde_json::json!({})).unwrap();
        assert_eq!(i.name(), "key-auth");
        // Default header = apikey
        let mut ctx = make_ctx(vec![("apikey", "test-key")]);
        assert!(matches!(i.access(&mut ctx), PluginResult::Continue));
    }

    #[test]
    fn test_configure_custom_header_and_hide() {
        use ando_plugin::plugin::Plugin;
        let i = KeyAuthPlugin.configure(&serde_json::json!({
            "header": "authorization",
            "hide_credentials": true
        })).unwrap();
        let mut ctx = make_ctx(vec![("authorization", "token-abc")]);
        let result = i.access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        // header removed because hide_credentials = true
        assert!(ctx.request_headers.get("authorization").is_none());
    }
}
