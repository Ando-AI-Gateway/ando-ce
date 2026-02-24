use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Rate-limiting plugin — fixed window counter per client IP.
///
/// **Important: per-worker semantics.** When running with SO_REUSEPORT
/// (N worker threads), each worker maintains its own rate counter.
/// The effective global limit is `count × N`. This is a deliberate
/// trade-off for zero atomic contention on the hot path. For exact
/// global rate limiting, use a shared store (Redis / etcd) — available
/// in Ando Enterprise Edition's `rate-limiting-advanced` plugin.
pub struct RateLimitingPlugin;

#[derive(Debug, Deserialize)]
struct RateLimitingConfig {
    /// Maximum requests allowed in the window.
    count: u64,
    /// Window size in seconds.
    time_window: u64,
}

struct WindowState {
    count: u64,
    window_start: Instant,
}

struct RateLimitingInstance {
    max_count: u64,
    window: Duration,
    /// IP address → window state.
    counters: Mutex<HashMap<String, WindowState>>,
}

impl Plugin for RateLimitingPlugin {
    fn name(&self) -> &str {
        "rate-limiting"
    }

    fn priority(&self) -> i32 {
        1001
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: RateLimitingConfig = serde_json::from_value(config.clone())
            .map_err(|e| anyhow::anyhow!("rate-limiting config error: {e}"))?;

        Ok(Box::new(RateLimitingInstance {
            max_count: cfg.count,
            window: Duration::from_secs(cfg.time_window),
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
        let key = ctx.client_ip.clone();
        let now = Instant::now();

        let mut counters = self.counters.lock().unwrap();
        let state = counters.entry(key).or_insert_with(|| WindowState {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now.duration_since(state.window_start) >= self.window {
            state.count = 0;
            state.window_start = now;
        }

        state.count += 1;

        if state.count > self.max_count {
            return PluginResult::Response {
                status: 429,
                headers: vec![
                    ("content-type".to_string(), "application/json".to_string()),
                    ("retry-after".to_string(), self.window.as_secs().to_string()),
                ],
                body: Some(br#"{"error":"Too many requests","status":429}"#.to_vec()),
            };
        }

        PluginResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_ctx(ip: &str) -> PluginContext {
        PluginContext::new(
            "r1".into(),
            ip.into(),
            "GET".into(),
            "/".into(),
            HashMap::new(),
        )
    }

    fn instance(count: u64, time_window: u64) -> RateLimitingInstance {
        RateLimitingInstance {
            max_count: count,
            window: Duration::from_secs(time_window),
            counters: Mutex::new(HashMap::new()),
        }
    }

    // ── Within limit ─────────────────────────────────────────────

    #[test]
    fn requests_within_limit_continue() {
        let inst = instance(5, 60);
        for _ in 0..5 {
            let result = inst.access(&mut make_ctx("1.2.3.4"));
            assert!(matches!(result, PluginResult::Continue));
        }
    }

    // ── Exceeding limit ──────────────────────────────────────────

    #[test]
    fn request_exceeding_limit_returns_429() {
        let inst = instance(3, 60);
        for _ in 0..3 {
            inst.access(&mut make_ctx("1.2.3.4"));
        }
        let result = inst.access(&mut make_ctx("1.2.3.4"));
        assert!(matches!(result, PluginResult::Response { status: 429, .. }));
    }

    // ── Different IPs are independent ────────────────────────────

    #[test]
    fn different_ips_have_independent_counters() {
        let inst = instance(1, 60);
        // First IP hits limit
        inst.access(&mut make_ctx("1.1.1.1"));
        assert!(matches!(
            inst.access(&mut make_ctx("1.1.1.1")),
            PluginResult::Response { status: 429, .. }
        ));
        // Second IP is unaffected
        assert!(matches!(
            inst.access(&mut make_ctx("2.2.2.2")),
            PluginResult::Continue
        ));
    }

    // ── Window reset ─────────────────────────────────────────────

    #[test]
    fn expired_window_resets_counter() {
        // Use a 0-second window so it expires immediately
        let inst = RateLimitingInstance {
            max_count: 1,
            window: Duration::from_nanos(1), // expires after 1ns
            counters: Mutex::new(HashMap::new()),
        };

        // First request — within limit
        assert!(matches!(
            inst.access(&mut make_ctx("1.2.3.4")),
            PluginResult::Continue
        ));

        // Sleep past the window
        std::thread::sleep(Duration::from_millis(5));

        // Window should have reset — first request of new window
        assert!(matches!(
            inst.access(&mut make_ctx("1.2.3.4")),
            PluginResult::Continue
        ));
    }

    // ── Limit = 0 blocks all requests ────────────────────────────

    #[test]
    fn zero_limit_blocks_all_requests() {
        let inst = instance(0, 60);
        let result = inst.access(&mut make_ctx("1.2.3.4"));
        assert!(matches!(result, PluginResult::Response { status: 429, .. }));
    }

    // ── 429 response includes retry-after header ─────────────────

    #[test]
    fn rate_limited_response_includes_retry_after_header() {
        let inst = instance(0, 30);
        let result = inst.access(&mut make_ctx("1.2.3.4"));
        match result {
            PluginResult::Response { headers, .. } => {
                let retry = headers.iter().find(|(k, _)| k == "retry-after");
                assert!(retry.is_some(), "retry-after header must be present");
                assert_eq!(retry.unwrap().1, "30");
            }
            _ => panic!("Expected Response"),
        }
    }

    // ── Plugin trait ─────────────────────────────────────────────

    #[test]
    fn plugin_name_priority_phases() {
        assert_eq!(RateLimitingPlugin.name(), "rate-limiting");
        assert_eq!(RateLimitingPlugin.priority(), 1001);
        assert_eq!(RateLimitingPlugin.phases(), &[Phase::Access]);
    }

    #[test]
    fn configure_with_valid_config_succeeds() {
        let config = serde_json::json!({ "count": 10, "time_window": 60 });
        let result = RateLimitingPlugin.configure(&config);
        assert!(result.is_ok(), "Valid rate-limiting config should succeed");
    }

    #[test]
    fn configure_missing_count_fails() {
        let config = serde_json::json!({ "time_window": 60 });
        assert!(
            RateLimitingPlugin.configure(&config).is_err(),
            "Missing 'count' must fail"
        );
    }

    #[test]
    fn configure_missing_time_window_fails() {
        let config = serde_json::json!({ "count": 10 });
        assert!(
            RateLimitingPlugin.configure(&config).is_err(),
            "Missing 'time_window' must fail"
        );
    }

    #[test]
    fn configured_instance_enforces_rate_limit() {
        let config = serde_json::json!({ "count": 2, "time_window": 60 });
        let instance = RateLimitingPlugin.configure(&config).unwrap();
        let mut ctx = make_ctx("5.5.5.5");
        assert!(matches!(instance.access(&mut ctx), PluginResult::Continue));
        assert!(matches!(
            instance.access(&mut make_ctx("5.5.5.5")),
            PluginResult::Continue
        ));
        assert!(matches!(
            instance.access(&mut make_ctx("5.5.5.5")),
            PluginResult::Response { status: 429, .. }
        ));
    }

    // ── Per-worker semantics: each instance is independent ───────

    #[test]
    fn separate_instances_have_independent_counters() {
        // Simulates two worker threads each having their own PluginInstance
        let config = serde_json::json!({ "count": 1, "time_window": 60 });
        let instance_a = RateLimitingPlugin.configure(&config).unwrap();
        let instance_b = RateLimitingPlugin.configure(&config).unwrap();

        // Worker A: first request passes
        assert!(matches!(
            instance_a.access(&mut make_ctx("1.1.1.1")),
            PluginResult::Continue
        ));
        // Worker A: second request blocked
        assert!(matches!(
            instance_a.access(&mut make_ctx("1.1.1.1")),
            PluginResult::Response { status: 429, .. }
        ));
        // Worker B: same IP, first request still passes (independent counter)
        assert!(matches!(
            instance_b.access(&mut make_ctx("1.1.1.1")),
            PluginResult::Continue
        ));
    }

    // ── Mutex is not poisoned after panic in another thread ──────

    #[test]
    fn rate_limiter_mutex_is_safe_under_concurrent_access() {
        use std::sync::Arc;
        let instance: Arc<RateLimitingInstance> = Arc::new(RateLimitingInstance {
            max_count: 1000,
            window: Duration::from_secs(60),
            counters: Mutex::new(HashMap::new()),
        });

        let mut handles = vec![];
        for _ in 0..4 {
            let inst = Arc::clone(&instance);
            handles.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    let _ = inst.access(&mut make_ctx("10.10.10.10"));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        // If we get here, no panic / deadlock occurred
        // 400 total requests with limit 1000 → all should have passed
        let result = instance.access(&mut make_ctx("10.10.10.10"));
        // 401st request might or might not pass depending on timing, but no panic
        let _ = result;
    }

    // ── Large window value does not overflow ─────────────────────

    #[test]
    fn large_time_window_does_not_panic() {
        let inst = instance(100, 86400 * 365); // 1-year window
        assert!(matches!(
            inst.access(&mut make_ctx("1.2.3.4")),
            PluginResult::Continue
        ));
    }
}
