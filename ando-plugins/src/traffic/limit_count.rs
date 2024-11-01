use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Rate limiting plugin using a fixed-window counter.
///
/// Configuration:
/// ```json
/// {
///   "count": 100,
///   "time_window": 60,
///   "key": "remote_addr",
///   "rejected_code": 429,
///   "rejected_msg": "Too many requests"
/// }
/// ```
pub struct LimitCountPlugin {
    /// Counters: key -> (count, window_start)
    counters: Arc<DashMap<String, (u64, Instant)>>,
}

impl LimitCountPlugin {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl Plugin for LimitCountPlugin {
    fn name(&self) -> &str {
        "limit-count"
    }

    fn priority(&self) -> i32 {
        1002
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
        let count_limit = config
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(100);

        let time_window = config
            .get("time_window")
            .and_then(|v| v.as_u64())
            .unwrap_or(60);

        let rejected_code = config
            .get("rejected_code")
            .and_then(|v| v.as_u64())
            .unwrap_or(429) as u16;

        let rejected_msg = config
            .get("rejected_msg")
            .and_then(|v| v.as_str())
            .unwrap_or("Too many requests");

        // Determine the rate limit key
        let key_type = config
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("remote_addr");

        let key = match key_type {
            "remote_addr" => format!("limit:{}:{}", ctx.route_id, ctx.client_ip),
            "consumer" => {
                let consumer = ctx
                    .get_var("jwt_sub")
                    .and_then(|v| v.as_str())
                    .or_else(|| ctx.get_var("api_key").and_then(|v| v.as_str()))
                    .unwrap_or("anonymous");
                format!("limit:{}:{}", ctx.route_id, consumer)
            }
            _ => format!("limit:{}:{}", ctx.route_id, ctx.client_ip),
        };

        let now = Instant::now();
        let window_duration = std::time::Duration::from_secs(time_window);

        let mut entry = self.counters.entry(key.clone()).or_insert((0, now));
        let (ref mut count, ref mut window_start) = *entry;

        // Check if window has expired
        if now.duration_since(*window_start) >= window_duration {
            *count = 0;
            *window_start = now;
        }

        *count += 1;
        let current_count = *count;
        let remaining = count_limit.saturating_sub(current_count);

        // Set rate limit headers
        ctx.set_response_header(
            "X-RateLimit-Limit".to_string(),
            count_limit.to_string(),
        );
        ctx.set_response_header(
            "X-RateLimit-Remaining".to_string(),
            remaining.to_string(),
        );

        if current_count > count_limit {
            return PluginResult::Response {
                status: rejected_code,
                headers: HashMap::from([
                    ("content-type".to_string(), "application/json".to_string()),
                    ("X-RateLimit-Limit".to_string(), count_limit.to_string()),
                    ("X-RateLimit-Remaining".to_string(), "0".to_string()),
                    ("Retry-After".to_string(), time_window.to_string()),
                ]),
                body: Some(
                    format!(
                        r#"{{"error":"{}","status":{}}}"#,
                        rejected_msg, rejected_code
                    )
                    .into_bytes(),
                ),
            };
        }

        PluginResult::Continue
    }
}
