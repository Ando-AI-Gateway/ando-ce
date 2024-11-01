use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use ipnet::IpNet;
use serde_json::Value;
use std::collections::HashMap;
use std::net::IpAddr;

/// IP restriction plugin — allow or deny based on client IP.
///
/// Configuration:
/// ```json
/// {
///   "whitelist": ["192.168.0.0/16", "10.0.0.0/8"],
///   "blacklist": ["1.2.3.4"],
///   "message": "IP not allowed"
/// }
/// ```
pub struct IpRestrictionPlugin;

impl IpRestrictionPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for IpRestrictionPlugin {
    fn name(&self) -> &str {
        "ip-restriction"
    }

    fn priority(&self) -> i32 {
        3000
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
        let client_ip = match ctx.client_ip.parse::<IpAddr>() {
            Ok(ip) => ip,
            Err(_) => {
                // Try stripping port
                let stripped = ctx.client_ip.split(':').next().unwrap_or(&ctx.client_ip);
                match stripped.parse::<IpAddr>() {
                    Ok(ip) => ip,
                    Err(_) => return PluginResult::Continue, // Can't parse IP, allow
                }
            }
        };

        let message = config
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Your IP address is not allowed");

        // Check whitelist (if set, ONLY these IPs are allowed)
        if let Some(whitelist) = config.get("whitelist").and_then(|v| v.as_array()) {
            let allowed = whitelist.iter().any(|cidr| {
                cidr.as_str()
                    .and_then(|s| s.parse::<IpNet>().ok())
                    .map(|net| net.contains(&client_ip))
                    .unwrap_or(false)
            });

            if !allowed {
                return PluginResult::Response {
                    status: 403,
                    headers: HashMap::from([
                        ("content-type".to_string(), "application/json".to_string()),
                    ]),
                    body: Some(
                        format!(r#"{{"error":"{}","status":403}}"#, message).into_bytes(),
                    ),
                };
            }
        }

        // Check blacklist
        if let Some(blacklist) = config.get("blacklist").and_then(|v| v.as_array()) {
            let blocked = blacklist.iter().any(|cidr| {
                cidr.as_str()
                    .and_then(|s| s.parse::<IpNet>().ok())
                    .map(|net| net.contains(&client_ip))
                    .unwrap_or(false)
            });

            if blocked {
                return PluginResult::Response {
                    status: 403,
                    headers: HashMap::from([
                        ("content-type".to_string(), "application/json".to_string()),
                    ]),
                    body: Some(
                        format!(r#"{{"error":"{}","status":403}}"#, message).into_bytes(),
                    ),
                };
            }
        }

        PluginResult::Continue
    }
}
