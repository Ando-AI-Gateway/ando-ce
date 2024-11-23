use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// Key Authentication plugin.
///
/// Authenticates requests using an API key passed via header or query parameter.
/// The key is validated against registered consumers in the cache.
///
/// Configuration (on the route/service):
/// ```json
/// { "header": "apikey", "query": "apikey", "hide_credentials": false }
/// ```
///
/// Consumer credential (on the consumer):
/// ```json
/// { "key-auth": { "key": "my-secret-key" } }
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
            .unwrap_or("apikey");

        let query_name = config
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("apikey");

        let hide_credentials = config
            .get("hide_credentials")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // 1. Extract the key from header or query string
        let key = ctx.get_header(header_name).map(|s| s.to_string())
            .or_else(|| ctx.get_header(&format!("x-{header_name}")).map(|s| s.to_string()))
            .or_else(|| {
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
                    ("www-authenticate".to_string(), format!("Key realm=\"{header_name}\"")),
                ]),
                body: Some(
                    r#"{"error":"Missing API key","status":401}"#
                        .as_bytes()
                        .to_vec(),
                ),
            };
        };

        // 2. Validate against consumers in the cache
        let matched_consumer = ctx.consumers.values().find(|consumer| {
            consumer
                .plugins
                .get("key-auth")
                .and_then(|cfg| cfg.get("key"))
                .and_then(|k| k.as_str())
                .map(|k| k == api_key)
                .unwrap_or(false)
        });

        let Some(consumer) = matched_consumer.cloned() else {
            return PluginResult::Response {
                status: 401,
                headers: HashMap::from([
                    ("content-type".to_string(), "application/json".to_string()),
                ]),
                body: Some(
                    r#"{"error":"Invalid API key","status":401}"#
                        .as_bytes()
                        .to_vec(),
                ),
            };
        };

        // 3. Attach consumer to context (available to downstream plugins)
        ctx.set_var(
            "consumer".to_string(),
            serde_json::Value::String(consumer.username.clone()),
        );
        ctx.consumer = Some(consumer.clone());

        // 4. Optionally strip the credential header from the upstream request
        if hide_credentials {
            ctx.remove_header(header_name);
        }

        PluginResult::Continue
    }
}
