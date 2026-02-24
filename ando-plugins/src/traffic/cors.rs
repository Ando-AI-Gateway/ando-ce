use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use serde::Deserialize;

pub struct CorsPlugin;

#[derive(Debug, Deserialize, Clone)]
struct CorsConfig {
    #[serde(default = "default_allow_origins")]
    allow_origins: Vec<String>,
    #[serde(default = "default_allow_methods")]
    allow_methods: Vec<String>,
    #[serde(default = "default_allow_headers")]
    allow_headers: Vec<String>,
    #[serde(default)]
    allow_credentials: bool,
    #[serde(default = "default_max_age")]
    max_age: u32,
}

fn default_allow_origins() -> Vec<String> {
    vec!["*".to_string()]
}
fn default_allow_methods() -> Vec<String> {
    vec![
        "GET".to_string(),
        "POST".to_string(),
        "PUT".to_string(),
        "DELETE".to_string(),
        "OPTIONS".to_string(),
    ]
}
fn default_allow_headers() -> Vec<String> {
    vec!["*".to_string()]
}
fn default_max_age() -> u32 {
    5
}

struct CorsInstance {
    cfg: CorsConfig,
}

impl Plugin for CorsPlugin {
    fn name(&self) -> &str {
        "cors"
    }

    fn priority(&self) -> i32 {
        2000
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: CorsConfig = serde_json::from_value(config.clone())
            .map_err(|e| anyhow::anyhow!("cors config error: {e}"))?;
        Ok(Box::new(CorsInstance { cfg }))
    }
}

impl CorsInstance {
    /// Returns the matched origin string, or None if origin is disallowed.
    fn resolve_origin(&self, origin: &str) -> Option<String> {
        if self.cfg.allow_origins.iter().any(|o| o == "*") {
            return Some("*".to_string());
        }
        if self.cfg.allow_origins.iter().any(|o| o == origin) {
            return Some(origin.to_string());
        }
        None
    }

    fn cors_headers(&self, origin_value: &str) -> Vec<(String, String)> {
        let mut h = vec![
            (
                "access-control-allow-origin".to_string(),
                origin_value.to_string(),
            ),
            (
                "access-control-allow-methods".to_string(),
                self.cfg.allow_methods.join(", "),
            ),
            (
                "access-control-allow-headers".to_string(),
                self.cfg.allow_headers.join(", "),
            ),
            (
                "access-control-max-age".to_string(),
                self.cfg.max_age.to_string(),
            ),
        ];
        if self.cfg.allow_credentials {
            h.push((
                "access-control-allow-credentials".to_string(),
                "true".to_string(),
            ));
        }
        h
    }
}

impl PluginInstance for CorsInstance {
    fn name(&self) -> &str {
        "cors"
    }

    fn priority(&self) -> i32 {
        2000
    }

    fn access(&self, ctx: &mut PluginContext) -> PluginResult {
        let origin = match ctx.request_headers.get("origin") {
            Some(o) => o.clone(),
            None => return PluginResult::Continue, // not a CORS request
        };

        let resolved = match self.resolve_origin(&origin) {
            Some(o) => o,
            None => {
                return PluginResult::Response {
                    status: 403,
                    headers: vec![("content-type".to_string(), "application/json".to_string())],
                    body: Some(
                        br#"{"error":"Forbidden - origin not allowed","status":403}"#.to_vec(),
                    ),
                };
            }
        };

        // Preflight
        if ctx.method == "OPTIONS" {
            let mut headers = self.cors_headers(&resolved);
            headers.push(("content-length".to_string(), "0".to_string()));
            return PluginResult::Response {
                status: 204,
                headers,
                body: None,
            };
        }

        // Add CORS headers to context variables for response phase (simple request)
        for (k, v) in self.cors_headers(&resolved) {
            ctx.vars
                .insert(format!("_cors_{k}"), serde_json::Value::String(v));
        }

        PluginResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_ctx(method: &str, origin: Option<&str>) -> PluginContext {
        let mut headers = HashMap::new();
        if let Some(o) = origin {
            headers.insert("origin".to_string(), o.to_string());
        }
        PluginContext::new(
            "r1".into(),
            "1.2.3.4".into(),
            method.into(),
            "/api".into(),
            headers,
        )
    }

    fn instance(config: serde_json::Value) -> CorsInstance {
        let cfg: CorsConfig = serde_json::from_value(config).unwrap();
        CorsInstance { cfg }
    }

    // ── No origin header → pass through ─────────────────────────

    #[test]
    fn no_origin_header_passes_through() {
        let inst = instance(serde_json::json!({}));
        let result = inst.access(&mut make_ctx("GET", None));
        assert!(matches!(result, PluginResult::Continue));
    }

    // ── Wildcard origin allows any origin ────────────────────────

    #[test]
    fn wildcard_allows_any_origin() {
        let inst = instance(serde_json::json!({ "allow_origins": ["*"] }));
        let result = inst.access(&mut make_ctx("GET", Some("https://evil.com")));
        assert!(matches!(result, PluginResult::Continue));
    }

    // ── Specific origin list allows matching origin ───────────────

    #[test]
    fn specific_origin_list_allows_matching() {
        let inst = instance(serde_json::json!({
            "allow_origins": ["https://example.com", "https://app.example.com"]
        }));
        let result = inst.access(&mut make_ctx("GET", Some("https://example.com")));
        assert!(matches!(result, PluginResult::Continue));
    }

    // ── Disallowed origin returns 403 ────────────────────────────

    #[test]
    fn disallowed_origin_returns_403() {
        let inst = instance(serde_json::json!({
            "allow_origins": ["https://example.com"]
        }));
        let result = inst.access(&mut make_ctx("GET", Some("https://evil.com")));
        assert!(matches!(result, PluginResult::Response { status: 403, .. }));
    }

    // ── OPTIONS preflight returns 204 ────────────────────────────

    #[test]
    fn options_preflight_returns_204() {
        let inst = instance(serde_json::json!({}));
        let result = inst.access(&mut make_ctx("OPTIONS", Some("https://example.com")));
        assert!(matches!(result, PluginResult::Response { status: 204, .. }));
    }

    // ── Preflight contains CORS headers ──────────────────────────

    #[test]
    fn preflight_response_has_cors_headers() {
        let inst = instance(serde_json::json!({
            "allow_methods": ["GET", "POST"],
            "allow_headers": ["Content-Type"]
        }));
        let result = inst.access(&mut make_ctx("OPTIONS", Some("https://example.com")));
        match result {
            PluginResult::Response { headers, .. } => {
                assert!(
                    headers
                        .iter()
                        .any(|(k, _)| k == "access-control-allow-methods")
                );
                assert!(
                    headers
                        .iter()
                        .any(|(k, _)| k == "access-control-allow-headers")
                );
            }
            _ => panic!("Expected Response"),
        }
    }

    // ── allow_credentials appends header ────────────────────────

    #[test]
    fn allow_credentials_adds_header() {
        let inst = instance(serde_json::json!({
            "allow_credentials": true,
            "allow_origins": ["https://example.com"]
        }));
        let result = inst.access(&mut make_ctx("OPTIONS", Some("https://example.com")));
        match result {
            PluginResult::Response { headers, .. } => {
                let cred = headers
                    .iter()
                    .find(|(k, _)| k == "access-control-allow-credentials");
                assert!(cred.is_some());
                assert_eq!(cred.unwrap().1, "true");
            }
            _ => panic!("Expected Response"),
        }
    }

    // ── Simple GET request continues and stores CORS vars ────────

    #[test]
    fn simple_get_continues_and_stores_cors_vars() {
        let inst = instance(serde_json::json!({}));
        let mut ctx = make_ctx("GET", Some("https://example.com"));
        let result = inst.access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert!(ctx.vars.contains_key("_cors_access-control-allow-origin"));
    }

    // ── Plugin trait ─────────────────────────────────────────────

    #[test]
    fn plugin_name_priority_phases() {
        assert_eq!(CorsPlugin.name(), "cors");
        assert_eq!(CorsPlugin.priority(), 2000);
        assert_eq!(CorsPlugin.phases(), &[Phase::Access]);
    }

    #[test]
    fn configure_empty_config_succeeds() {
        let result = CorsPlugin.configure(&serde_json::json!({}));
        assert!(
            result.is_ok(),
            "Empty cors config should succeed (all defaults)"
        );
    }

    #[test]
    fn configure_with_origins_succeeds() {
        let config = serde_json::json!({
            "allow_origins": ["https://example.com", "https://app.example.com"],
            "allow_credentials": true
        });
        let result = CorsPlugin.configure(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn configure_with_invalid_type_fails() {
        // allow_origins should be an array, not a string
        let config = serde_json::json!({ "allow_origins": "not-an-array" });
        assert!(CorsPlugin.configure(&config).is_err());
    }

    // ── Preflight: max-age is present and correct ────────────────

    #[test]
    fn preflight_response_includes_max_age() {
        let inst = instance(serde_json::json!({ "max_age": 600 }));
        let result = inst.access(&mut make_ctx("OPTIONS", Some("https://example.com")));
        match result {
            PluginResult::Response { headers, .. } => {
                let max_age = headers.iter().find(|(k, _)| k == "access-control-max-age");
                assert!(max_age.is_some(), "max-age must be present");
                assert_eq!(max_age.unwrap().1, "600");
            }
            _ => panic!("Expected preflight Response"),
        }
    }

    // ── Preflight: content-length: 0 header ───────────────────────

    #[test]
    fn preflight_response_has_zero_content_length() {
        let inst = instance(serde_json::json!({}));
        let result = inst.access(&mut make_ctx("OPTIONS", Some("https://example.com")));
        match result {
            PluginResult::Response { headers, .. } => {
                let cl = headers.iter().find(|(k, _)| k == "content-length");
                assert!(cl.is_some(), "content-length must be present in preflight");
                assert_eq!(cl.unwrap().1, "0");
            }
            _ => panic!("Expected preflight Response"),
        }
    }

    // ── Preflight: disallowed origin returns 403, not 204 ────────

    #[test]
    fn preflight_disallowed_origin_returns_403() {
        let inst = instance(serde_json::json!({
            "allow_origins": ["https://good.com"]
        }));
        let result = inst.access(&mut make_ctx("OPTIONS", Some("https://evil.com")));
        assert!(
            matches!(result, PluginResult::Response { status: 403, .. }),
            "OPTIONS with disallowed origin must return 403, not 204"
        );
    }

    // ── Simple request stores all CORS vars in context ────────────

    #[test]
    fn simple_request_stores_all_cors_headers_in_vars() {
        let inst = instance(serde_json::json!({
            "allow_methods": ["GET", "POST"],
            "allow_headers": ["Content-Type"],
            "allow_credentials": true,
            "max_age": 3600,
            "allow_origins": ["https://example.com"]
        }));
        let mut ctx = make_ctx("GET", Some("https://example.com"));
        let result = inst.access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert!(ctx.vars.contains_key("_cors_access-control-allow-origin"));
        assert!(ctx.vars.contains_key("_cors_access-control-allow-methods"));
        assert!(ctx.vars.contains_key("_cors_access-control-allow-headers"));
        assert!(ctx.vars.contains_key("_cors_access-control-max-age"));
        assert!(
            ctx.vars
                .contains_key("_cors_access-control-allow-credentials")
        );
    }

    // ── Wildcard origin: reflected as "*" not the actual origin ────

    #[test]
    fn wildcard_origin_reflects_star_not_actual_origin() {
        let inst = instance(serde_json::json!({})); // defaults to allow_origins: ["*"]
        let mut ctx = make_ctx("GET", Some("https://specific-origin.com"));
        inst.access(&mut ctx);
        let origin_val = ctx
            .vars
            .get("_cors_access-control-allow-origin")
            .and_then(|v| v.as_str());
        assert_eq!(
            origin_val,
            Some("*"),
            "wildcard config should reflect '*' not the actual origin"
        );
    }

    // ── Specific origin: reflected as the matched origin ──────────

    #[test]
    fn specific_origin_reflects_actual_origin() {
        let inst = instance(serde_json::json!({
            "allow_origins": ["https://example.com"]
        }));
        let mut ctx = make_ctx("GET", Some("https://example.com"));
        inst.access(&mut ctx);
        let origin_val = ctx
            .vars
            .get("_cors_access-control-allow-origin")
            .and_then(|v| v.as_str());
        assert_eq!(
            origin_val,
            Some("https://example.com"),
            "specific allow_origins should reflect the actual origin"
        );
    }

    // ── Credentials not added when false ──────────────────────────

    #[test]
    fn no_credentials_header_when_allow_credentials_false() {
        let inst = instance(serde_json::json!({
            "allow_credentials": false,
            "allow_origins": ["https://example.com"]
        }));
        let result = inst.access(&mut make_ctx("OPTIONS", Some("https://example.com")));
        match result {
            PluginResult::Response { headers, .. } => {
                let cred = headers
                    .iter()
                    .find(|(k, _)| k == "access-control-allow-credentials");
                assert!(
                    cred.is_none(),
                    "allow-credentials header should not be sent when false"
                );
            }
            _ => panic!("Expected Response"),
        }
    }
}
