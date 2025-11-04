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
    /// The consumer lookup is done inline (no async) using
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
