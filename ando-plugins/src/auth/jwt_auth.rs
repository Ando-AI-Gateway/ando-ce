use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

pub struct JwtAuthPlugin;

#[derive(Debug, Deserialize)]
struct JwtAuthConfig {
    /// Secret for HMAC algorithms (HS256 / HS384 / HS512).
    #[serde(default)]
    secret: Option<String>,
    /// PEM/DER public key for RS/EC algorithms.
    #[serde(default)]
    public_key: Option<String>,
    /// Algorithm — default "HS256".
    #[serde(default = "default_algorithm")]
    algorithm: String,
    /// Header that carries the token — default "authorization".
    #[serde(default = "default_header")]
    header: String,
}

fn default_algorithm() -> String {
    "HS256".to_string()
}
fn default_header() -> String {
    "authorization".to_string()
}

/// Minimal JWT claims we extract.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    sub: Option<String>,
    exp: Option<u64>,
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

struct JwtAuthInstance {
    decoding_key: DecodingKey,
    validation: Validation,
    header: String,
}

impl Plugin for JwtAuthPlugin {
    fn name(&self) -> &str {
        "jwt-auth"
    }

    fn priority(&self) -> i32 {
        2510
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: JwtAuthConfig = serde_json::from_value(config.clone())
            .map_err(|e| anyhow::anyhow!("jwt-auth config error: {e}"))?;

        let algorithm: Algorithm = cfg
            .algorithm
            .parse()
            .map_err(|_| anyhow::anyhow!("unknown JWT algorithm: {}", cfg.algorithm))?;

        let decoding_key = match algorithm {
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
                let secret = cfg
                    .secret
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("jwt-auth requires 'secret' for HMAC algorithms"))?;
                DecodingKey::from_secret(secret.as_bytes())
            }
            _ => {
                let key = cfg
                    .public_key
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("jwt-auth requires 'public_key' for asymmetric algorithms"))?;
                DecodingKey::from_rsa_pem(key.as_bytes())
                    .map_err(|e| anyhow::anyhow!("invalid public key: {e}"))?
            }
        };

        let mut validation = Validation::new(algorithm);
        validation.validate_exp = true;

        Ok(Box::new(JwtAuthInstance {
            decoding_key,
            validation,
            header: cfg.header,
        }))
    }
}

impl PluginInstance for JwtAuthInstance {
    fn name(&self) -> &str {
        "jwt-auth"
    }

    fn priority(&self) -> i32 {
        2510
    }

    fn access(&self, ctx: &mut PluginContext) -> PluginResult {
        let raw_header = match ctx.request_headers.get(&self.header) {
            Some(h) => h.clone(),
            None => {
                return deny_401(br#"{"error":"Missing Authorization header","status":401}"#);
            }
        };

        let token = if raw_header.to_lowercase().starts_with("bearer ") {
            raw_header[7..].trim()
        } else {
            raw_header.trim()
        };

        let data = match decode::<Claims>(token, &self.decoding_key, &self.validation) {
            Ok(d) => d,
            Err(e) => {
                let msg = format!(
                    "{{\"error\":\"Invalid token: {}\",\"status\":401}}",
                    e.to_string().replace('"', "'")
                );
                return PluginResult::Response {
                    status: 401,
                    headers: vec![
                        ("content-type".to_string(), "application/json".to_string()),
                        ("www-authenticate".to_string(), "Bearer".to_string()),
                    ],
                    body: Some(msg.into_bytes()),
                };
            }
        };

        if let Some(sub) = data.claims.sub {
            ctx.consumer = Some(sub.clone());
            ctx.vars.insert("_jwt_sub".to_string(), serde_json::Value::String(sub));
        }

        PluginResult::Continue
    }
}

fn deny_401(body: &'static [u8]) -> PluginResult {
    PluginResult::Response {
        status: 401,
        headers: vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("www-authenticate".to_string(), "Bearer".to_string()),
        ],
        body: Some(body.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use jsonwebtoken::{encode, EncodingKey, Header};

    const SECRET: &str = "test-secret-key";

    fn make_ctx(auth: Option<&str>) -> PluginContext {
        let mut headers = HashMap::new();
        if let Some(v) = auth {
            headers.insert("authorization".to_string(), v.to_string());
        }
        PluginContext::new("r1".into(), "1.2.3.4".into(), "GET".into(), "/api".into(), headers)
    }

    fn make_instance() -> JwtAuthInstance {
        JwtAuthInstance {
            decoding_key: DecodingKey::from_secret(SECRET.as_bytes()),
            validation: Validation::new(Algorithm::HS256),
            header: "authorization".to_string(),
        }
    }

    fn make_token(sub: &str, exp_offset_secs: i64) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let exp = (now + exp_offset_secs) as u64;

        let claims = serde_json::json!({
            "sub": sub,
            "exp": exp,
        });
        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap()
    }

    // ── Missing header returns 401 ───────────────────────────────

    #[test]
    fn missing_header_returns_401() {
        let inst = make_instance();
        let result = inst.access(&mut make_ctx(None));
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    // ── Valid token continues ────────────────────────────────────

    #[test]
    fn valid_token_continues_and_sets_consumer() {
        let inst = make_instance();
        let token = make_token("alice", 3600);
        let mut ctx = make_ctx(Some(&format!("Bearer {token}")));
        let result = inst.access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.consumer.as_deref(), Some("alice"));
    }

    // ── Expired token returns 401 ────────────────────────────────

    #[test]
    fn expired_token_returns_401() {
        let inst = make_instance();
        let token = make_token("alice", -3600); // expired 1 hour ago
        let result = inst.access(&mut make_ctx(Some(&format!("Bearer {token}"))));
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    // ── Wrong secret returns 401 ─────────────────────────────────

    #[test]
    fn wrong_secret_returns_401() {
        let inst = JwtAuthInstance {
            decoding_key: DecodingKey::from_secret(b"wrong-secret"),
            validation: Validation::new(Algorithm::HS256),
            header: "authorization".to_string(),
        };
        let token = make_token("alice", 3600);
        let result = inst.access(&mut make_ctx(Some(&format!("Bearer {token}"))));
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    // ── Token without Bearer prefix also works ───────────────────

    #[test]
    fn token_without_bearer_prefix_also_accepted() {
        let inst = make_instance();
        let token = make_token("bob", 3600);
        let mut ctx = make_ctx(Some(&token));
        let result = inst.access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.consumer.as_deref(), Some("bob"));
    }

    // ── Malformed token string returns 401 ───────────────────────

    #[test]
    fn malformed_token_returns_401() {
        let inst = make_instance();
        let result = inst.access(&mut make_ctx(Some("Bearer not.a.jwt")));
        assert!(matches!(result, PluginResult::Response { status: 401, .. }));
    }

    // ── Valid token sets _jwt_sub var ────────────────────────────

    #[test]
    fn valid_token_sets_jwt_sub_var() {
        let inst = make_instance();
        let token = make_token("charlie", 3600);
        let mut ctx = make_ctx(Some(&format!("Bearer {token}")));
        inst.access(&mut ctx);
        assert_eq!(
            ctx.vars.get("_jwt_sub"),
            Some(&serde_json::Value::String("charlie".to_string()))
        );
    }

    // ── Plugin trait: name / priority / phases ───────────────────

    #[test]
    fn plugin_name_priority_phases() {
        assert_eq!(JwtAuthPlugin.name(), "jwt-auth");
        assert_eq!(JwtAuthPlugin.priority(), 2510);
        assert_eq!(JwtAuthPlugin.phases(), &[Phase::Access]);
    }

    // ── configure() — success paths ──────────────────────────────

    #[test]
    fn configure_hs256_with_secret_succeeds() {
        let config = serde_json::json!({ "secret": "my-secret", "algorithm": "HS256" });
        assert!(JwtAuthPlugin.configure(&config).is_ok());
    }

    #[test]
    fn configure_hs384_with_secret_succeeds() {
        let config = serde_json::json!({ "secret": "my-secret", "algorithm": "HS384" });
        assert!(JwtAuthPlugin.configure(&config).is_ok());
    }

    #[test]
    fn configure_hs512_with_secret_succeeds() {
        let config = serde_json::json!({ "secret": "my-secret", "algorithm": "HS512" });
        assert!(JwtAuthPlugin.configure(&config).is_ok());
    }

    #[test]
    fn configure_with_defaults_uses_hs256() {
        // Minimal config — algorithm and header use defaults
        let config = serde_json::json!({ "secret": SECRET });
        let instance = JwtAuthPlugin.configure(&config).unwrap();
        // Verify default header "authorization" works
        let token = make_token("default-user", 3600);
        let mut ctx = make_ctx(Some(&format!("Bearer {token}")));
        let result = instance.access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.consumer.as_deref(), Some("default-user"));
    }

    #[test]
    fn configure_with_custom_header() {
        let config = serde_json::json!({ "secret": SECRET, "header": "x-token" });
        let instance = JwtAuthPlugin.configure(&config).unwrap();
        let token = make_token("user", 3600);
        let mut headers = HashMap::new();
        headers.insert("x-token".to_string(), format!("Bearer {token}"));
        let mut ctx = PluginContext::new("r1".into(), "1.2.3.4".into(), "GET".into(), "/".into(), headers);
        assert!(matches!(instance.access(&mut ctx), PluginResult::Continue));
    }

    // ── configure() — failure paths ──────────────────────────────

    #[test]
    fn configure_unknown_algorithm_fails() {
        let config = serde_json::json!({ "secret": "my-secret", "algorithm": "INVALID" });
        assert!(JwtAuthPlugin.configure(&config).is_err());
    }

    #[test]
    fn configure_hmac_without_secret_fails() {
        let config = serde_json::json!({ "algorithm": "HS256" });
        let Err(err) = JwtAuthPlugin.configure(&config) else {
            panic!("expected configure to fail");
        };
        assert!(err.to_string().contains("secret"), "error should mention 'secret': {err}");
    }

    #[test]
    fn configure_with_invalid_json_type_fails() {
        let config = serde_json::json!({ "algorithm": 12345 });
        assert!(JwtAuthPlugin.configure(&config).is_err());
    }

    // ── Token with no sub claim — no consumer set ─────────────────

    #[test]
    fn token_without_sub_does_not_set_consumer() {
        let inst = make_instance();
        // Build token with no sub field
        use std::time::{SystemTime, UNIX_EPOCH};
        let exp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 3600;
        let claims = serde_json::json!({ "exp": exp, "role": "admin" });
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        ).unwrap();
        let mut ctx = make_ctx(Some(&format!("Bearer {token}")));
        let result = inst.access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert!(ctx.consumer.is_none());
    }

    // ── JWT edge cases: nbf (not-before) ─────────────────────────

    #[test]
    fn token_with_nbf_in_future_is_rejected() {
        // jsonwebtoken validates nbf if present
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let claims = serde_json::json!({
            "sub": "alice",
            "exp": now + 7200,
            "nbf": now + 3600, // not valid for another hour
        });
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        ).unwrap();

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        let inst = JwtAuthInstance {
            decoding_key: DecodingKey::from_secret(SECRET.as_bytes()),
            validation,
            header: "authorization".to_string(),
        };
        let result = inst.access(&mut make_ctx(Some(&format!("Bearer {token}"))));
        assert!(matches!(result, PluginResult::Response { status: 401, .. }),
            "token with nbf in the future should be rejected");
    }

    // ── JWT edge cases: algorithm mismatch ────────────────────────

    #[test]
    fn token_signed_with_hs384_rejected_by_hs256_instance() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let claims = serde_json::json!({ "sub": "alice", "exp": now + 3600 });
        // Sign with HS384
        let token = encode(
            &Header::new(Algorithm::HS384),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        ).unwrap();
        // Validate expecting HS256
        let inst = make_instance(); // HS256
        let result = inst.access(&mut make_ctx(Some(&format!("Bearer {token}"))));
        assert!(matches!(result, PluginResult::Response { status: 401, .. }),
            "HS384 token must be rejected by HS256 validator");
    }

    // ── JWT edge cases: empty token string ────────────────────────

    #[test]
    fn empty_bearer_token_returns_401() {
        let inst = make_instance();
        let result = inst.access(&mut make_ctx(Some("Bearer ")));
        assert!(matches!(result, PluginResult::Response { status: 401, .. }),
            "empty bearer token must be rejected");
    }

    #[test]
    fn empty_string_header_returns_401() {
        let inst = make_instance();
        let result = inst.access(&mut make_ctx(Some("")));
        assert!(matches!(result, PluginResult::Response { status: 401, .. }),
            "empty string header must be rejected");
    }

    // ── JWT: 401 response includes WWW-Authenticate header ───────

    #[test]
    fn jwt_401_response_includes_www_authenticate() {
        let inst = make_instance();
        let result = inst.access(&mut make_ctx(None));
        match result {
            PluginResult::Response { headers, status, .. } => {
                assert_eq!(status, 401);
                let www_auth = headers.iter().find(|(k, _)| k == "www-authenticate");
                assert!(www_auth.is_some(), "401 must include www-authenticate header");
                assert_eq!(www_auth.unwrap().1, "Bearer");
            }
            _ => panic!("Expected 401 Response"),
        }
    }

    // ── JWT: token with extra whitespace ──────────────────────────

    #[test]
    fn bearer_token_with_extra_whitespace_is_trimmed() {
        let inst = make_instance();
        let token = make_token("alice", 3600);
        let mut ctx = make_ctx(Some(&format!("Bearer   {token}  ")));
        let result = inst.access(&mut ctx);
        assert!(matches!(result, PluginResult::Continue),
            "token with surrounding whitespace should be accepted");
        assert_eq!(ctx.consumer.as_deref(), Some("alice"));
    }
}
