use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use serde_json::Value;

/// Response transformer plugin.
pub struct ResponseTransformerPlugin;

impl ResponseTransformerPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for ResponseTransformerPlugin {
    fn name(&self) -> &str {
        "response-transformer"
    }

    fn priority(&self) -> i32 {
        3001
    }

    fn phases(&self) -> Vec<Phase> {
        vec![Phase::HeaderFilter]
    }

    async fn execute(
        &self,
        _phase: Phase,
        ctx: &mut PluginContext,
        config: &Value,
    ) -> PluginResult {
        // Add response headers
        if let Some(add) = config.get("add").and_then(|v| v.get("headers")) {
            if let Some(obj) = add.as_object() {
                for (k, v) in obj {
                    if let Some(val) = v.as_str() {
                        ctx.set_response_header(k.clone(), val.to_string());
                    }
                }
            }
        }

        // Remove response headers
        if let Some(remove) = config.get("remove").and_then(|v| v.get("headers")) {
            if let Some(arr) = remove.as_array() {
                for v in arr {
                    if let Some(name) = v.as_str() {
                        ctx.response_headers.remove(name);
                    }
                }
            }
        }

        PluginResult::Continue
    }
}
