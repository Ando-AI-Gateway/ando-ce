use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// Basic HTTP Authentication plugin.
pub struct BasicAuthPlugin;

impl BasicAuthPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for BasicAuthPlugin {
    fn name(&self) -> &str {
        "basic-auth"
    }

    fn priority(&self) -> i32 {
        2520
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
        let auth_header = match ctx.get_header("authorization") {
            Some(h) => h.to_string(),
            None => {
                return PluginResult::Response {
                    status: 401,
                    headers: HashMap::from([
                        ("www-authenticate".to_string(), "Basic realm=\"ando\"".to_string()),
                        ("content-type".to_string(), "application/json".to_string()),
                    ]),
                    body: Some(
                        r#"{"error":"Missing credentials","status":401}"#.as_bytes().to_vec(),
                    ),
                };
            }
        };

        let encoded = match auth_header.strip_prefix("Basic ") {
            Some(e) => e,
            None => {
                return PluginResult::Response {
                    status: 401,
                    headers: HashMap::from([
                        ("content-type".to_string(), "application/json".to_string()),
                    ]),
                    body: Some(
                        r#"{"error":"Invalid Basic auth format","status":401}"#.as_bytes().to_vec(),
                    ),
                };
            }
        };

        // Decode base64
        // TODO: use proper base64 crate
        ctx.set_var(
            "basic_auth_encoded".to_string(),
            Value::String(encoded.to_string()),
        );

        let hide = config
            .get("hide_credentials")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if hide {
            ctx.remove_header("authorization");
        }

        PluginResult::Continue
    }
}
