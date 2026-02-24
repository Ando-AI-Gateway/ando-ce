//! Security Headers plugin.
//!
//! Injects standardised HTTP response headers that protect clients from
//! common web-layer attacks and satisfy the transmission-security controls
//! mandated by the main compliance frameworks supported by Ando CE.
//!
//! | Standard              | Control                                               |
//! |-----------------------|-------------------------------------------------------|
//! | HIPAA                 | 164.312(e)(1) – Transmission Security (HSTS, no-store)|
//! | SOC2 Type II          | CC6.6 – Logical access controls over transmission     |
//! | ISO/IEC 27001:2022    | A.8.23 – Web filtering / A.8.24 – Cryptography        |
//! | GDPR                  | Art. 32 – technical measures (TLS + header hardening) |
//! | OWASP Top 10          | A05:2021 Security Misconfiguration                    |
//!
//! # Example plugin config
//!
//! ```yaml
//! plugins:
//!   - name: security-headers
//!     config:
//!       hsts_max_age: 31536000
//!       hsts_include_subdomains: true
//!       hsts_preload: true
//!       x_frame_options: "DENY"
//!       x_content_type_options: true
//!       referrer_policy: "no-referrer"
//!       content_security_policy: "default-src 'self'"
//!       permissions_policy: "geolocation=(), microphone=(), camera=()"
//!       no_store_cache: false
//! ```

use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use serde::Deserialize;

// ─────────────────────────────────────────────────────────────
// Config
// ─────────────────────────────────────────────────────────────

/// Configuration for the security-headers plugin.
///
/// All fields have secure defaults so the plugin is effective with an
/// empty `config: {}` block.
#[derive(Debug, Deserialize, Clone)]
struct SecurityHeadersConfig {
    /// `Strict-Transport-Security: max-age=<N>` value (seconds).
    /// Default: 31 536 000 (1 year — Chrome preload list minimum).
    #[serde(default = "default_hsts_max_age")]
    hsts_max_age: u64,

    /// Append `; includeSubDomains` to the HSTS directive.
    #[serde(default = "default_true")]
    hsts_include_subdomains: bool,

    /// Append `; preload` to the HSTS directive (required for HSTS preload list).
    #[serde(default = "default_true")]
    hsts_preload: bool,

    /// `X-Frame-Options` value: `DENY`, `SAMEORIGIN`, or empty string to omit.
    #[serde(default = "default_frame_options")]
    x_frame_options: String,

    /// Emit `X-Content-Type-Options: nosniff`.
    #[serde(default = "default_true")]
    x_content_type_options: bool,

    /// `Referrer-Policy` value.
    #[serde(default = "default_referrer_policy")]
    referrer_policy: String,

    /// `Content-Security-Policy` value.  Empty string = omit the header.
    #[serde(default)]
    content_security_policy: String,

    /// `Permissions-Policy` value.  Empty string = omit the header.
    #[serde(default = "default_permissions_policy")]
    permissions_policy: String,

    /// Emit `Cache-Control: no-store, no-cache` and `Pragma: no-cache`.
    /// **Required for PHI / PII routes** (HIPAA 164.312(e)(2)(i)).
    #[serde(default)]
    no_store_cache: bool,
}

fn default_hsts_max_age() -> u64 {
    31_536_000
}
fn default_true() -> bool {
    true
}
fn default_frame_options() -> String {
    "DENY".into()
}
fn default_referrer_policy() -> String {
    "no-referrer".into()
}
fn default_permissions_policy() -> String {
    "geolocation=(), microphone=(), camera=()".into()
}

// ─────────────────────────────────────────────────────────────
// Plugin factory
// ─────────────────────────────────────────────────────────────

/// Security Headers plugin factory.
pub struct SecurityHeadersPlugin;

impl Plugin for SecurityHeadersPlugin {
    fn name(&self) -> &str {
        "security-headers"
    }

    /// Priority 3000: runs before most other header-filter plugins so that
    /// downstream plugins can override individual headers if necessary.
    fn priority(&self) -> i32 {
        3000
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::HeaderFilter]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: SecurityHeadersConfig = serde_json::from_value(config.clone())
            .map_err(|e| anyhow::anyhow!("security-headers config error: {e}"))?;

        let headers = build_headers(&cfg);
        Ok(Box::new(SecurityHeadersInstance { headers }))
    }
}

// ─────────────────────────────────────────────────────────────
// Instance
// ─────────────────────────────────────────────────────────────

struct SecurityHeadersInstance {
    /// Pre-built list of (name, value) pairs to inject.  Calculated once at
    /// configure-time to avoid per-request allocation.
    headers: Vec<(String, String)>,
}

impl PluginInstance for SecurityHeadersInstance {
    fn name(&self) -> &str {
        "security-headers"
    }
    fn priority(&self) -> i32 {
        3000
    }

    fn header_filter(&self, ctx: &mut PluginContext) -> PluginResult {
        for (k, v) in &self.headers {
            ctx.response_headers.insert(k.clone(), v.clone());
        }
        PluginResult::Continue
    }
}

/// Build the flat list of response headers from a validated config.
///
/// Extracted as a free function to allow unit-testing header generation
/// without constructing a full plugin instance.
fn build_headers(cfg: &SecurityHeadersConfig) -> Vec<(String, String)> {
    let mut h: Vec<(String, String)> = Vec::with_capacity(8);

    // ── HSTS (HIPAA 164.312(e)(1), SOC2 CC6.6, ISO A.8.24) ──────
    let mut hsts = format!("max-age={}", cfg.hsts_max_age);
    if cfg.hsts_include_subdomains {
        hsts.push_str("; includeSubDomains");
    }
    if cfg.hsts_preload {
        hsts.push_str("; preload");
    }
    h.push(("strict-transport-security".into(), hsts));

    // ── Clickjacking (OWASP A05) ─────────────────────────────────
    if !cfg.x_frame_options.is_empty() {
        h.push(("x-frame-options".into(), cfg.x_frame_options.clone()));
    }

    // ── MIME sniffing ────────────────────────────────────────────
    if cfg.x_content_type_options {
        h.push(("x-content-type-options".into(), "nosniff".into()));
    }

    // ── Legacy XSS filter (IE / old Chrome) ─────────────────────
    h.push(("x-xss-protection".into(), "1; mode=block".into()));

    // ── Referrer leakage ────────────────────────────────────────
    if !cfg.referrer_policy.is_empty() {
        h.push(("referrer-policy".into(), cfg.referrer_policy.clone()));
    }

    // ── Content-Security-Policy ──────────────────────────────────
    if !cfg.content_security_policy.is_empty() {
        h.push((
            "content-security-policy".into(),
            cfg.content_security_policy.clone(),
        ));
    }

    // ── Permissions-Policy ───────────────────────────────────────
    if !cfg.permissions_policy.is_empty() {
        h.push(("permissions-policy".into(), cfg.permissions_policy.clone()));
    }

    // ── Cache-Control (PHI / PII routes — HIPAA) ─────────────────
    if cfg.no_store_cache {
        h.push(("cache-control".into(), "no-store, no-cache".into()));
        h.push(("pragma".into(), "no-cache".into()));
    }

    h
}

// ─────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> SecurityHeadersConfig {
        serde_json::from_value(serde_json::json!({})).unwrap()
    }

    fn cfg_with(extra: serde_json::Value) -> SecurityHeadersConfig {
        serde_json::from_value(extra).unwrap()
    }

    // ── HSTS ─────────────────────────────────────────────────────

    #[test]
    fn hsts_header_present_with_default_config() {
        let hdrs = build_headers(&default_cfg());
        let hsts = hdrs.iter().find(|(k, _)| k == "strict-transport-security");
        assert!(hsts.is_some(), "HSTS header must be present");
        let val = &hsts.unwrap().1;
        assert!(val.contains("max-age=31536000"));
        assert!(val.contains("includeSubDomains"));
        assert!(val.contains("preload"));
    }

    #[test]
    fn hsts_without_subdomains_and_preload() {
        let cfg = cfg_with(serde_json::json!({
            "hsts_include_subdomains": false,
            "hsts_preload": false
        }));
        let hdrs = build_headers(&cfg);
        let val = &hdrs
            .iter()
            .find(|(k, _)| k == "strict-transport-security")
            .unwrap()
            .1;
        assert!(!val.contains("includeSubDomains"));
        assert!(!val.contains("preload"));
    }

    #[test]
    fn custom_hsts_max_age() {
        let cfg = cfg_with(serde_json::json!({ "hsts_max_age": 63072000 }));
        let hdrs = build_headers(&cfg);
        let val = &hdrs
            .iter()
            .find(|(k, _)| k == "strict-transport-security")
            .unwrap()
            .1;
        assert!(val.contains("max-age=63072000"));
    }

    // ── X-Frame-Options ──────────────────────────────────────────

    #[test]
    fn x_frame_options_defaults_to_deny() {
        let hdrs = build_headers(&default_cfg());
        let xfo = hdrs.iter().find(|(k, _)| k == "x-frame-options");
        assert_eq!(xfo.map(|(_, v)| v.as_str()), Some("DENY"));
    }

    #[test]
    fn x_frame_options_sameorigin() {
        let cfg = cfg_with(serde_json::json!({ "x_frame_options": "SAMEORIGIN" }));
        let hdrs = build_headers(&cfg);
        let xfo = hdrs.iter().find(|(k, _)| k == "x-frame-options").unwrap();
        assert_eq!(xfo.1, "SAMEORIGIN");
    }

    #[test]
    fn x_frame_options_omitted_when_empty() {
        let cfg = cfg_with(serde_json::json!({ "x_frame_options": "" }));
        let hdrs = build_headers(&cfg);
        assert!(hdrs.iter().all(|(k, _)| k != "x-frame-options"));
    }

    // ── X-Content-Type-Options ───────────────────────────────────

    #[test]
    fn x_content_type_options_nosniff_by_default() {
        let hdrs = build_headers(&default_cfg());
        let xcto = hdrs.iter().find(|(k, _)| k == "x-content-type-options");
        assert_eq!(xcto.map(|(_, v)| v.as_str()), Some("nosniff"));
    }

    #[test]
    fn x_content_type_options_omitted_when_disabled() {
        let cfg = cfg_with(serde_json::json!({ "x_content_type_options": false }));
        let hdrs = build_headers(&cfg);
        assert!(hdrs.iter().all(|(k, _)| k != "x-content-type-options"));
    }

    // ── Referrer-Policy ──────────────────────────────────────────

    #[test]
    fn referrer_policy_defaults_to_no_referrer() {
        let hdrs = build_headers(&default_cfg());
        let rp = hdrs.iter().find(|(k, _)| k == "referrer-policy").unwrap();
        assert_eq!(rp.1, "no-referrer");
    }

    // ── Content-Security-Policy ──────────────────────────────────

    #[test]
    fn csp_omitted_by_default() {
        let hdrs = build_headers(&default_cfg());
        assert!(hdrs.iter().all(|(k, _)| k != "content-security-policy"));
    }

    #[test]
    fn csp_injected_when_configured() {
        let cfg = cfg_with(serde_json::json!({
            "content_security_policy": "default-src 'self'"
        }));
        let hdrs = build_headers(&cfg);
        let csp = hdrs
            .iter()
            .find(|(k, _)| k == "content-security-policy")
            .unwrap();
        assert_eq!(csp.1, "default-src 'self'");
    }

    // ── Cache-Control / Pragma (PHI routes) ──────────────────────

    #[test]
    fn cache_control_omitted_by_default() {
        let hdrs = build_headers(&default_cfg());
        assert!(hdrs.iter().all(|(k, _)| k != "cache-control"));
    }

    #[test]
    fn cache_control_no_store_when_enabled() {
        let cfg = cfg_with(serde_json::json!({ "no_store_cache": true }));
        let hdrs = build_headers(&cfg);
        let cc = hdrs.iter().find(|(k, _)| k == "cache-control").unwrap();
        assert_eq!(cc.1, "no-store, no-cache");
        let pragma = hdrs.iter().find(|(k, _)| k == "pragma").unwrap();
        assert_eq!(pragma.1, "no-cache");
    }

    // ── Plugin interface ─────────────────────────────────────────

    #[test]
    fn configure_with_empty_config_succeeds() {
        let plugin = SecurityHeadersPlugin;
        let instance = plugin.configure(&serde_json::json!({}));
        assert!(instance.is_ok());
    }

    #[test]
    fn configure_with_invalid_config_returns_error() {
        let plugin = SecurityHeadersPlugin;
        let result = plugin.configure(&serde_json::json!({ "hsts_max_age": "not-a-number" }));
        assert!(result.is_err());
    }

    #[test]
    fn header_filter_inserts_into_context() {
        use std::collections::HashMap;
        let plugin = SecurityHeadersPlugin;
        let instance = plugin.configure(&serde_json::json!({})).unwrap();
        let mut ctx = PluginContext::new(
            "route-1".into(),
            "127.0.0.1".into(),
            "GET".into(),
            "/".into(),
            HashMap::new(),
        );
        let result = instance.header_filter(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert!(
            ctx.response_headers
                .contains_key("strict-transport-security")
        );
        assert!(ctx.response_headers.contains_key("x-frame-options"));
        assert!(ctx.response_headers.contains_key("x-content-type-options"));
    }

    #[test]
    fn x_xss_protection_always_present() {
        let hdrs = build_headers(&default_cfg());
        let xss = hdrs.iter().find(|(k, _)| k == "x-xss-protection");
        assert_eq!(xss.map(|(_, v)| v.as_str()), Some("1; mode=block"));
    }

    #[test]
    fn permissions_policy_defaults_disable_sensors() {
        let hdrs = build_headers(&default_cfg());
        let pp = hdrs
            .iter()
            .find(|(k, _)| k == "permissions-policy")
            .unwrap();
        assert!(pp.1.contains("geolocation=()"));
        assert!(pp.1.contains("microphone=()"));
        assert!(pp.1.contains("camera=()"));
    }
}
