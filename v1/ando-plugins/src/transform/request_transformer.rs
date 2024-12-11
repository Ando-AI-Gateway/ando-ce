use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use serde_json::Value;

/// Request transformer plugin — add/remove/rename headers and query params.
pub struct RequestTransformerPlugin;

impl RequestTransformerPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for RequestTransformerPlugin {
    fn name(&self) -> &str {
        "request-transformer"
    }

    fn priority(&self) -> i32 {
        3000
    }

    fn phases(&self) -> Vec<Phase> {
        vec![Phase::Rewrite]
    }

    async fn execute(
        &self,
        _phase: Phase,
        ctx: &mut PluginContext,
        config: &Value,
    ) -> PluginResult {
        // Add headers
        if let Some(add) = config.get("add").and_then(|v| v.get("headers")) {
            if let Some(obj) = add.as_object() {
                for (k, v) in obj {
                    if let Some(val) = v.as_str() {
                        ctx.set_header(k.clone(), val.to_string());
                    }
                }
            }
        }

        // Remove headers
        if let Some(remove) = config.get("remove").and_then(|v| v.get("headers")) {
            if let Some(arr) = remove.as_array() {
                for v in arr {
                    if let Some(name) = v.as_str() {
                        ctx.remove_header(name);
                    }
                }
            }
        }

        // Rename headers
        if let Some(rename) = config.get("rename").and_then(|v| v.get("headers")) {
            if let Some(obj) = rename.as_object() {
                for (old_name, new_name) in obj {
                    if let Some(new) = new_name.as_str() {
                        if let Some(value) = ctx.get_header(old_name).map(|s| s.to_string()) {
                            ctx.remove_header(old_name);
                            ctx.set_header(new.to_string(), value);
                        }
                    }
                }
            }
        }

        PluginResult::Continue
    }
}
