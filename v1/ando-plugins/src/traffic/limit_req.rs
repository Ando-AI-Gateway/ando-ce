use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use serde_json::Value;

/// Request rate limiter using leaky bucket algorithm.
pub struct LimitReqPlugin;

impl LimitReqPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for LimitReqPlugin {
    fn name(&self) -> &str {
        "limit-req"
    }

    fn priority(&self) -> i32 {
        1001
    }

    fn phases(&self) -> Vec<Phase> {
        vec![Phase::Access]
    }

    async fn execute(
        &self,
        _phase: Phase,
        _ctx: &mut PluginContext,
        _config: &Value,
    ) -> PluginResult {
        // TODO: Implement leaky bucket algorithm
        PluginResult::Continue
    }
}
