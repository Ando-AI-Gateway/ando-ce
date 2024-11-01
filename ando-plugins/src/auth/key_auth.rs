use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// Key Authentication plugin.
///
/// Authenticates requests using an API key passed via header or query parameter.
/// Configuration:
/// ```json
/// {
///   "header": "X-API-KEY",
///   "query": "apikey",
///   "hide_credentials": true
/// }
/// ```
pub struct KeyAuthPlugin;

impl KeyAuthPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for KeyAuthPlugin {
    fn name(&self) -> &str {
        "key-auth"
    }

    fn priority(&self) -> i32 {
        2500
    }

    fn phases(&self) -> Vec<Phase> {
        vec![Phase::Access]
    }

    async fn execute(
        &self,
        _phase: Phase,
        ctx: &mut PluginContext,
        config: &Value,
    ) -> PluginResult {
        let header_name = config
            .get("header")
            .and_then(|v| v.as_str())
            .unwrap_or("X-API-KEY");

        let query_name = config
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("apikey");

        let hide_credentials = config
            .get("hide_credentials")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Try to get API key from header
        let key = ctx.get_header(header_name).map(|s| s.to_string());

        // Try query string if not in header
        let key = key.or_else(|| {
            ctx.request_query
                .split('&')
                .find_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    let k = parts.next()?;
                    let v = parts.next()?;
                    if k == query_name {
                        Some(v.to_string())
                    } else {
                        None
                    }
                })
        });

        let Some(api_key) = key else {
            return PluginResult::Response {
                status: 401,
                headers: HashMap::from([
                    ("content-type".to_string(), "application/json".to_string()),
                ]),
                body: Some(
                    r#"{"error":"Missing API key","status":401}"#
                        .as_bytes()
                        .to_vec(),
                ),
            };
        };

        // TODO: Validate the key against consumers in the cache
        // For now, store the key in context for downstream plugins
        ctx.set_var(
            "api_key".to_string(),
            serde_json::Value::String(api_key.clone()),
        );

        // Hide credentials if configured
        if hide_credentials {
            ctx.remove_header(header_name);
        }

        PluginResult::Continue
    }
}
