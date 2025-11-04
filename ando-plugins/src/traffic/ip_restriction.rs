use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use serde::Deserialize;
use std::net::IpAddr;

/// IP restriction plugin — allow/deny lists for client IPs.
///
/// Supports both IPv4 and IPv6, including CIDR notation.
/// Uses `whitelist` (allow) or `blacklist` (deny) mode.
pub struct IpRestrictionPlugin;

#[derive(Debug, Deserialize, Clone)]
struct IpRestrictionConfig {
    /// Allowed IPs/CIDRs — only these can access (if set, blacklist is ignored).
    #[serde(default)]
    whitelist: Vec<String>,
    /// Denied IPs/CIDRs.
    #[serde(default)]
    blacklist: Vec<String>,
    /// Custom rejection message.
    #[serde(default = "default_message")]
    message: String,
}

fn default_message() -> String {
    "Your IP address is not allowed".to_string()
}

struct IpRestrictionInstance {
    whitelist: Vec<ipnet::IpNet>,
    blacklist: Vec<ipnet::IpNet>,
    message: String,
}

impl Plugin for IpRestrictionPlugin {
    fn name(&self) -> &str {
        "ip-restriction"
    }

    fn priority(&self) -> i32 {
        3000 // APISIX default priority for ip-restriction
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: IpRestrictionConfig = serde_json::from_value(config.clone())?;

        let whitelist = cfg
            .whitelist
            .iter()
            .filter_map(|s| parse_ip_or_cidr(s))
            .collect();
        let blacklist = cfg
            .blacklist
            .iter()
            .filter_map(|s| parse_ip_or_cidr(s))
            .collect();

        Ok(Box::new(IpRestrictionInstance {
            whitelist,
            blacklist,
            message: cfg.message,
        }))
    }
}

fn parse_ip_or_cidr(s: &str) -> Option<ipnet::IpNet> {
    // Try parsing as CIDR first, then as bare IP
    s.parse::<ipnet::IpNet>().ok().or_else(|| {
        s.parse::<IpAddr>().ok().map(|ip| match ip {
            IpAddr::V4(v4) => ipnet::IpNet::V4(ipnet::Ipv4Net::new(v4, 32).unwrap()),
            IpAddr::V6(v6) => ipnet::IpNet::V6(ipnet::Ipv6Net::new(v6, 128).unwrap()),
        })
    })
}

impl PluginInstance for IpRestrictionInstance {
    fn name(&self) -> &str {
        "ip-restriction"
    }

    fn priority(&self) -> i32 {
        3000
    }

    fn access(&self, ctx: &mut PluginContext) -> PluginResult {
        let client_ip: IpAddr = match ctx.client_ip.parse() {
            Ok(ip) => ip,
            Err(_) => return PluginResult::Continue, // Can't parse, allow through
        };

        // Whitelist mode: only allow listed IPs
        if !self.whitelist.is_empty() {
            if !self.whitelist.iter().any(|net| net.contains(&client_ip)) {
                return PluginResult::Response {
                    status: 403,
                    headers: vec![
                        ("content-type".to_string(), "application/json".to_string()),
                    ],
                    body: Some(
                        format!(r#"{{"error":"{}","status":403}}"#, self.message).into_bytes(),
                    ),
                };
            }
            return PluginResult::Continue;
        }

        // Blacklist mode: deny listed IPs
        if self.blacklist.iter().any(|net| net.contains(&client_ip)) {
            return PluginResult::Response {
                status: 403,
                headers: vec![("content-type".to_string(), "application/json".to_string())],
                body: Some(
                    format!(r#"{{"error":"{}","status":403}}"#, self.message).into_bytes(),
                ),
            };
        }

        PluginResult::Continue
    }
}
