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
}
