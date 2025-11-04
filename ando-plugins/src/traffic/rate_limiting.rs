use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Local (in-memory) rate limiting plugin.
///
/// Implements a sliding-window counter per client IP or route.
/// For distributed rate limiting across multiple nodes, use the
/// Enterprise Edition's `rate-limiting-advanced` plugin with Redis backend.
pub struct RateLimitingPlugin;

#[derive(Debug, Deserialize, Clone)]
struct RateLimitingConfig {
    /// Requests per second.
    #[serde(default)]
    rate: Option<u64>,
    /// Requests per minute.
    #[serde(default)]
    rate_per_minute: Option<u64>,
    /// Requests per hour.
    #[serde(default)]
    rate_per_hour: Option<u64>,
    /// Limit by: "ip" (default) or "route".
    #[serde(default = "default_limit_by")]
    limit_by: String,
    /// Custom rejection message.
    #[serde(default = "default_message")]
    message: String,
}

fn default_limit_by() -> String {
    "ip".to_string()
}

fn default_message() -> String {
    "Rate limit exceeded".to_string()
}

struct RateLimitingInstance {
    /// Max requests in the window.
    max_requests: u64,
    /// Window duration in seconds.
    window_secs: u64,
    /// Limit by "ip" or "route".
    limit_by: String,
    /// Custom rejection message.
    message: String,
    /// Sliding window counters: key → (count, window_start).
    counters: Mutex<HashMap<String, (u64, Instant)>>,
}

impl Plugin for RateLimitingPlugin {
    fn name(&self) -> &str {
        "rate-limiting"
    }

    fn priority(&self) -> i32 {
        1001 // APISIX default priority for limit-req
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: RateLimitingConfig = serde_json::from_value(config.clone())?;

        // Resolve rate configuration (priority: rate > rate_per_minute > rate_per_hour)
        let (max_requests, window_secs) = if let Some(rps) = cfg.rate {
            (rps, 1u64)
        } else if let Some(rpm) = cfg.rate_per_minute {
            (rpm, 60)
        } else if let Some(rph) = cfg.rate_per_hour {
            (rph, 3600)
        } else {
            // Default: 60 requests per minute
            (60, 60)
        };

        Ok(Box::new(RateLimitingInstance {
            max_requests,
            window_secs,
            limit_by: cfg.limit_by,
            message: cfg.message,
            counters: Mutex::new(HashMap::new()),
        }))
    }
}

impl PluginInstance for RateLimitingInstance {
    fn name(&self) -> &str {
        "rate-limiting"
    }

    fn priority(&self) -> i32 {
        1001
    }

    fn access(&self, ctx: &mut PluginContext) -> PluginResult {
        let key = match self.limit_by.as_str() {
            "route" => ctx.route_id.clone(),
            _ => ctx.client_ip.clone(), // default: per-IP
        };

        let now = Instant::now();
        let mut counters = match self.counters.lock() {
            Ok(guard) => guard,
            Err(_) => return PluginResult::Continue, // Poisoned mutex, allow through
        };

        let entry = counters.entry(key).or_insert((0, now));

        // If window has expired, reset
        if now.duration_since(entry.1).as_secs() >= self.window_secs {
            entry.0 = 0;
            entry.1 = now;
        }

        entry.0 += 1;

        if entry.0 > self.max_requests {
            let remaining_secs = self
                .window_secs
                .saturating_sub(now.duration_since(entry.1).as_secs());
            return PluginResult::Response {
                status: 429,
                headers: vec![
                    ("content-type".to_string(), "application/json".to_string()),
                    (
                        "x-ratelimit-limit".to_string(),
                        self.max_requests.to_string(),
                    ),
                    (
                        "x-ratelimit-remaining".to_string(),
                        "0".to_string(),
                    ),
                    (
                        "retry-after".to_string(),
                        remaining_secs.to_string(),
                    ),
                ],
                body: Some(
                    format!(r#"{{"error":"{}","status":429}}"#, self.message).into_bytes(),
                ),
            };
        }

        PluginResult::Continue
    }
}
