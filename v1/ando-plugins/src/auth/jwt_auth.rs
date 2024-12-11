use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// JWT Authentication plugin.
pub struct JwtAuthPlugin;

impl JwtAuthPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: Option<String>,
    exp: Option<usize>,
    iss: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[async_trait]
impl Plugin for JwtAuthPlugin {
    fn name(&self) -> &str {
        "jwt-auth"
    }

    fn priority(&self) -> i32 {
        2510
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
        let header = config
            .get("header")
            .and_then(|v| v.as_str())
            .unwrap_or("Authorization");

        let secret = match config.get("secret").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return PluginResult::Error("jwt-auth: missing 'secret' in config".to_string());
            }
        };

        // Extract token from header
        let token = match ctx.get_header(header) {
            Some(value) => {
                if let Some(token) = value.strip_prefix("Bearer ") {
                    token.to_string()
                } else {
                    value.to_string()
                }
            }
            None => {
                return PluginResult::Response {
                    status: 401,
                    headers: HashMap::from([
                        ("content-type".to_string(), "application/json".to_string()),
                    ]),
                    body: Some(
                        r#"{"error":"Missing JWT token","status":401}"#
                            .as_bytes()
                            .to_vec(),
                    ),
                };
            }
        };

        let algorithm = config
            .get("algorithm")
            .and_then(|v| v.as_str())
            .unwrap_or("HS256");

        let algo = match algorithm {
            "HS256" => Algorithm::HS256,
            "HS384" => Algorithm::HS384,
            "HS512" => Algorithm::HS512,
            "RS256" => Algorithm::RS256,
            _ => Algorithm::HS256,
        };

        let mut validation = Validation::new(algo);

        // Optionally validate issuer
        if let Some(iss) = config.get("issuer").and_then(|v| v.as_str()) {
            validation.set_issuer(&[iss]);
        }

        match decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        ) {
            Ok(token_data) => {
                // Store claims in context
                if let Some(sub) = &token_data.claims.sub {
                    ctx.set_var(
                        "jwt_sub".to_string(),
                        Value::String(sub.clone()),
                    );
                }
                ctx.set_var(
                    "jwt_claims".to_string(),
                    serde_json::to_value(&token_data.claims).unwrap_or_default(),
                );
                PluginResult::Continue
            }
            Err(e) => PluginResult::Response {
                status: 401,
                headers: HashMap::from([
                    ("content-type".to_string(), "application/json".to_string()),
                ]),
                body: Some(
                    format!(r#"{{"error":"Invalid JWT: {}","status":401}}"#, e)
                        .into_bytes(),
                ),
            },
        }
    }
}
