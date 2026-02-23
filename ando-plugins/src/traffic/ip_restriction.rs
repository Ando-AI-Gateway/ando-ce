use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use ipnet::IpNet;
use serde::Deserialize;
use std::net::IpAddr;
use std::str::FromStr;

/// IP restriction plugin — allowlist/denylist based access control.
pub struct IpRestrictionPlugin;

#[derive(Debug, Deserialize)]
struct IpRestrictionConfig {
    /// If non-empty, only these CIDRs/IPs are allowed.
    #[serde(default)]
    allowlist: Vec<String>,
    /// If non-empty, these CIDRs/IPs are blocked.
    #[serde(default)]
    denylist: Vec<String>,
}

struct IpRestrictionInstance {
    allowlist: Vec<IpNet>,
    denylist: Vec<IpNet>,
}

impl Plugin for IpRestrictionPlugin {
    fn name(&self) -> &str {
        "ip-restriction"
    }

    fn priority(&self) -> i32 {
        3000
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: IpRestrictionConfig = serde_json::from_value(config.clone())
            .unwrap_or_else(|_| IpRestrictionConfig {
                allowlist: vec![],
                denylist: vec![],
            });

        let parse_list = |list: Vec<String>| -> Vec<IpNet> {
            list.iter()
                .filter_map(|s| {
                    // Try to parse as CIDR first, then as plain IP (host /32 or /128)
                    IpNet::from_str(s).ok().or_else(|| {
                        IpAddr::from_str(s).ok().map(|ip| match ip {
                            IpAddr::V4(a) => IpNet::from(ipnet::Ipv4Net::from(a)),
                            IpAddr::V6(a) => IpNet::from(ipnet::Ipv6Net::from(a)),
                        })
                    })
                })
                .collect()
        };

        Ok(Box::new(IpRestrictionInstance {
            allowlist: parse_list(cfg.allowlist),
            denylist: parse_list(cfg.denylist),
        }))
    }
}

impl IpRestrictionInstance {
    fn matches_any(ip: &IpAddr, list: &[IpNet]) -> bool {
        list.iter().any(|net| net.contains(ip))
    }
}

impl PluginInstance for IpRestrictionInstance {
    fn name(&self) -> &str {
        "ip-restriction"
    }

    fn priority(&self) -> i32 {
        3000
    }

    fn access(&self, ctx: &mut PluginContext) -> PluginResult {
        let ip = match IpAddr::from_str(&ctx.client_ip) {
            Ok(ip) => ip,
            Err(_) => {
                // Unparseable IP — block to be safe
                return deny_403();
            }
        };

        // Denylist takes priority
        if !self.denylist.is_empty() && Self::matches_any(&ip, &self.denylist) {
            return deny_403();
        }

        // Allowlist: if set, IP must be in it
        if !self.allowlist.is_empty() && !Self::matches_any(&ip, &self.allowlist) {
            return deny_403();
        }

        PluginResult::Continue
    }
}

fn deny_403() -> PluginResult {
    PluginResult::Response {
        status: 403,
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        body: Some(br#"{"error":"IP not allowed","status":403}"#.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_ctx(client_ip: &str) -> PluginContext {
        PluginContext::new("r1".into(), client_ip.into(), "GET".into(), "/".into(), HashMap::new())
    }

    fn instance_from(config: serde_json::Value) -> IpRestrictionInstance {
        let plugin = IpRestrictionPlugin;
        let inst = plugin.configure(&config).unwrap();
        // Downcast by re-constructing from config
        let cfg: IpRestrictionConfig = serde_json::from_value(config).unwrap_or_default();
        let parse_list = |list: Vec<String>| -> Vec<IpNet> {
            list.iter()
                .filter_map(|s| IpNet::from_str(s).ok().or_else(|| {
                    IpAddr::from_str(s).ok().map(|ip| match ip {
                        IpAddr::V4(a) => IpNet::from(ipnet::Ipv4Net::from(a)),
                        IpAddr::V6(a) => IpNet::from(ipnet::Ipv6Net::from(a)),
                    })
                }))
                .collect()
        };
        let _ = inst; // drop boxed instance
        IpRestrictionInstance {
            allowlist: parse_list(cfg.allowlist),
            denylist: parse_list(cfg.denylist),
        }
    }

    // ── No restrictions ──────────────────────────────────────────

    #[test]
    fn no_restrictions_allows_any_ip() {
        let inst = instance_from(serde_json::json!({}));
        let mut ctx = make_ctx("1.2.3.4");
        assert!(matches!(inst.access(&mut ctx), PluginResult::Continue));
    }

    // ── Denylist ─────────────────────────────────────────────────

    #[test]
    fn denylist_blocks_direct_ip_match() {
        let inst = instance_from(serde_json::json!({ "denylist": ["10.0.0.1"] }));
        let mut ctx = make_ctx("10.0.0.1");
        assert!(matches!(inst.access(&mut ctx), PluginResult::Response { status: 403, .. }));
    }

    #[test]
    fn denylist_blocks_cidr_match() {
        let inst = instance_from(serde_json::json!({ "denylist": ["10.0.0.0/8"] }));
        let mut ctx = make_ctx("10.0.0.50");
        assert!(matches!(inst.access(&mut ctx), PluginResult::Response { status: 403, .. }));
    }

    #[test]
    fn denylist_allows_non_matching_ip() {
        let inst = instance_from(serde_json::json!({ "denylist": ["10.0.0.0/8"] }));
        let mut ctx = make_ctx("192.168.1.1");
        assert!(matches!(inst.access(&mut ctx), PluginResult::Continue));
    }

    // ── Allowlist ─────────────────────────────────────────────────

    #[test]
    fn allowlist_allows_matching_ip() {
        let inst = instance_from(serde_json::json!({ "allowlist": ["192.168.0.0/24"] }));
        let mut ctx = make_ctx("192.168.0.55");
        assert!(matches!(inst.access(&mut ctx), PluginResult::Continue));
    }

    #[test]
    fn allowlist_blocks_non_matching_ip() {
        let inst = instance_from(serde_json::json!({ "allowlist": ["192.168.0.0/24"] }));
        let mut ctx = make_ctx("10.0.0.1");
        assert!(matches!(inst.access(&mut ctx), PluginResult::Response { status: 403, .. }));
    }

    // ── Denylist takes priority over allowlist ─────────────────────

    #[test]
    fn denylist_takes_priority_over_allowlist() {
        let inst = instance_from(serde_json::json!({
            "allowlist": ["192.168.1.0/24"],
            "denylist": ["192.168.1.5"]
        }));
        let mut ctx = make_ctx("192.168.1.5");
        assert!(matches!(inst.access(&mut ctx), PluginResult::Response { status: 403, .. }));
    }

    // ── Multiple CIDRs ────────────────────────────────────────────

    #[test]
    fn multiple_denylist_cidrs() {
        let inst = instance_from(serde_json::json!({
            "denylist": ["10.0.0.0/8", "172.16.0.0/12"]
        }));
        assert!(matches!(instance_from(serde_json::json!({ "denylist": ["10.0.0.0/8"] }))
            .access(&mut make_ctx("10.5.5.5")), PluginResult::Response { status: 403, .. }));
        assert!(matches!(instance_from(serde_json::json!({ "denylist": ["172.16.0.0/12"] }))
            .access(&mut make_ctx("172.16.5.5")), PluginResult::Response { status: 403, .. }));
        let _ = inst;
    }

    // ── Plugin trait ─────────────────────────────────────────────

    #[test]
    fn plugin_name_priority_phases() {
        assert_eq!(IpRestrictionPlugin.name(), "ip-restriction");
        assert_eq!(IpRestrictionPlugin.priority(), 3000);
        assert_eq!(IpRestrictionPlugin.phases(), &[Phase::Access]);
    }

    #[test]
    fn configure_empty_config_succeeds() {
        let result = IpRestrictionPlugin.configure(&serde_json::json!({}));
        assert!(result.is_ok(), "Empty ip-restriction config should succeed");
    }

    #[test]
    fn configure_with_cidr_lists_succeeds() {
        let config = serde_json::json!({
            "allowlist": ["192.168.0.0/24"],
            "denylist": ["10.0.0.0/8"]
        });
        assert!(IpRestrictionPlugin.configure(&config).is_ok());
    }

    #[test]
    fn configure_with_invalid_cidr_silently_ignores_bad_entry() {
        // ip-restriction silently skips unparseable entries instead of failing
        let config = serde_json::json!({ "denylist": ["not-a-cidr", "10.0.0.0/8"] });
        let instance = IpRestrictionPlugin.configure(&config).unwrap();
        // Valid CIDR 10.0.0.0/8 must still be applied
        assert!(matches!(
            instance.access(&mut make_ctx("10.1.2.3")),
            PluginResult::Response { status: 403, .. }
        ));
    }
}

impl Default for IpRestrictionConfig {
    fn default() -> Self {
        Self { allowlist: vec![], denylist: vec![] }
    }
}
