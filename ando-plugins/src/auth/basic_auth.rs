use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

/// Basic-auth plugin — APISIX-compatible.
///
/// Extracts credentials from `Authorization: Basic <base64>` header and
/// stores them in `ctx.vars` for downstream consumer validation.
pub struct BasicAuthPlugin;

struct BasicAuthInstance;

impl Plugin for BasicAuthPlugin {
    fn name(&self) -> &str {
        "basic-auth"
    }

    fn priority(&self) -> i32 {
        2520 // Higher than key-auth (2500)
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, _config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        Ok(Box::new(BasicAuthInstance))
    }
}

impl PluginInstance for BasicAuthInstance {
    fn name(&self) -> &str {
        "basic-auth"
    }

    fn priority(&self) -> i32 {
        2520
    }

    fn access(&self, ctx: &mut PluginContext) -> PluginResult {
        let header = match ctx.get_header("authorization") {
            Some(h) => h.to_string(),
            None => {
                return deny_401(br#"{"error":"Missing authorization header","status":401}"#);
            }
        };

        let encoded = match header.strip_prefix("Basic ").or_else(|| header.strip_prefix("basic ")) {
            Some(e) => e,
            None => {
                return deny_401(br#"{"error":"Invalid authorization scheme","status":401}"#);
            }
        };

        let decoded = match BASE64.decode(encoded.trim()) {
            Ok(b) => b,
            Err(_) => {
                return deny_401(br#"{"error":"Invalid base64 encoding","status":401}"#);
            }
        };

        let credentials = match String::from_utf8(decoded) {
            Ok(s) => s,
            Err(_) => {
                return deny_401(br#"{"error":"Invalid credentials encoding","status":401}"#);
            }
        };

        let (username, password) = match credentials.split_once(':') {
            Some((u, p)) => (u.to_string(), p.to_string()),
            None => {
                return deny_401(br#"{"error":"Malformed credentials","status":401}"#);
            }
        };

        ctx.vars.insert("_basic_auth_user".to_string(), serde_json::Value::String(username));
        ctx.vars.insert("_basic_auth_pass".to_string(), serde_json::Value::String(password));

        PluginResult::Continue
    }
}

fn deny_401(body: &'static [u8]) -> PluginResult {
    PluginResult::Response {
        status: 401,
        headers: vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("www-authenticate".to_string(), r#"Basic realm="Ando""#.to_string()),
        ],
        body: Some(body.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn make_ctx(headers: Vec<(&str, &str)>) -> PluginContext {
        let map = headers
            .into_iter()
            .map(|(k, v)| (k.to_lowercase(), v.to_string()))
            .collect();
        PluginContext::new("r1".into(), "1.2.3.4".into(), "GET".into(), "/".into(), map)
    }

    fn instance() -> BasicAuthInstance {
        BasicAuthInstance
    }

    fn basic_header(user: &str, pass: &str) -> String {
        let creds = BASE64.encode(format!("{user}:{pass}"));
        format!("Basic {creds}")
    }

    // ── Missing header ───────────────────────────────────────────

    #[test]
    fn missing_authorization_header_returns_401() {
        let mut ctx = make_ctx(vec![]);
        let result = instance().access(&mut ctx);
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    // ── Wrong scheme ─────────────────────────────────────────────

    #[test]
    fn bearer_scheme_returns_401() {
        let mut ctx = make_ctx(vec![("authorization", "Bearer some-token")]);
        let result = instance().access(&mut ctx);
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    // ── Invalid base64 ───────────────────────────────────────────

    #[test]
    fn invalid_base64_returns_401() {
        let mut ctx = make_ctx(vec![("authorization", "Basic !!!invalid!!!")]);
        let result = instance().access(&mut ctx);
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    // ── Missing colon ─────────────────────────────────────────────

    #[test]
    fn credentials_without_colon_returns_401() {
        let encoded = BASE64.encode("justusername");
        let header = format!("Basic {encoded}");
        let mut ctx = make_ctx(vec![("authorization", &header)]);
        let result = instance().access(&mut ctx);
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    // ── Valid credentials ─────────────────────────────────────────

    #[test]
    fn valid_credentials_sets_vars_and_continues() {
        let header = basic_header("alice", "secret123");
        let mut ctx = make_ctx(vec![("authorization", &header)]);
        let result = instance().access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.vars["_basic_auth_user"], "alice");
        assert_eq!(ctx.vars["_basic_auth_pass"], "secret123");
    }

    #[test]
    fn password_with_colon_stores_full_password() {
        // password = "pass:with:colons"
        let header = basic_header("user", "pass:with:colons");
        let mut ctx = make_ctx(vec![("authorization", &header)]);
        instance().access(&mut ctx);
        assert_eq!(ctx.vars["_basic_auth_user"], "user");
        assert_eq!(ctx.vars["_basic_auth_pass"], "pass:with:colons");
    }

    #[test]
    fn empty_password_is_allowed() {
        let header = basic_header("user", "");
        let mut ctx = make_ctx(vec![("authorization", &header)]);
        let result = instance().access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.vars["_basic_auth_pass"], "");
    }

    #[test]
    fn case_insensitive_basic_prefix() {
        let encoded = BASE64.encode("user:pass");
        let header = format!("basic {encoded}"); // lowercase "basic"
        let mut ctx = make_ctx(vec![("authorization", &header)]);
        let result = instance().access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
    }

    // ── Plugin trait ─────────────────────────────────────────────

    #[test]
    fn plugin_name_priority_phases() {
        assert_eq!(BasicAuthPlugin.name(), "basic-auth");
        assert_eq!(BasicAuthPlugin.priority(), 2520);
        assert_eq!(BasicAuthPlugin.phases(), &[Phase::Access]);
    }

    #[test]
    fn configure_with_any_config_succeeds() {
        // BasicAuth accepts any config (no required fields)
        let instance = BasicAuthPlugin.configure(&serde_json::json!({})).unwrap();
        // Verify the instance works — missing header → 401
        let mut ctx = make_ctx(vec![]);
        assert!(matches!(instance.access(&mut ctx), PluginResult::Response { status: 401, .. }));
    }
}
