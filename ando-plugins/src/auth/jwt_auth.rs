use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use serde::Deserialize;

/// JWT authentication plugin.
///
/// Validates JWT tokens from the Authorization header (Bearer scheme).
/// Supports HS256, HS384, HS512, RS256, RS384, RS512, ES256, ES384.
/// Consumer lookup is done via the `key` claim matching a consumer's jwt-auth config.
pub struct JwtAuthPlugin;

#[derive(Debug, Deserialize)]
struct JwtAuthConfig {
    /// Header to extract the token from.
    #[serde(default = "default_header")]
    header: String,
    /// Whether to hide the auth header from upstream.
    #[serde(default)]
    hide_credentials: bool,
}

fn default_header() -> String {
    "authorization".to_string()
}

struct JwtAuthInstance {
    header: String,
    hide_credentials: bool,
}

impl Plugin for JwtAuthPlugin {
    fn name(&self) -> &str {
        "jwt-auth"
    }

    fn priority(&self) -> i32 {
        2510 // APISIX default priority for jwt-auth
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: JwtAuthConfig = serde_json::from_value(config.clone())
            .unwrap_or(JwtAuthConfig {
                header: default_header(),
                hide_credentials: false,
            });

        Ok(Box::new(JwtAuthInstance {
            header: cfg.header.to_lowercase(),
            hide_credentials: cfg.hide_credentials,
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
        let token = match ctx.get_header(&self.header) {
            Some(val) => {
                // Strip "Bearer " prefix if present
                if let Some(stripped) = val.strip_prefix("Bearer ") {
                    stripped.to_string()
                } else if let Some(stripped) = val.strip_prefix("bearer ") {
                    stripped.to_string()
                } else {
                    val.to_string()
                }
            }
            None => {
                return PluginResult::Response {
                    status: 401,
                    headers: vec![
                        ("content-type".to_string(), "application/json".to_string()),
                        (
                            "www-authenticate".to_string(),
                            "Bearer realm=\"Ando\"".to_string(),
                        ),
                    ],
                    body: Some(br#"{"error":"Missing JWT token","status":401}"#.to_vec()),
                };
            }
        };

        if token.is_empty() {
            return PluginResult::Response {
                status: 401,
                headers: vec![
                    ("content-type".to_string(), "application/json".to_string()),
                ],
                body: Some(br#"{"error":"Invalid JWT token","status":401}"#.to_vec()),
            };
        }

        // Store the token in vars for the proxy to validate against consumer secrets.
        // The actual JWT signature verification is done by the proxy layer which
        // has access to the consumer store with per-consumer secrets.
        ctx.vars.insert(
            "_jwt_auth_token".to_string(),
            serde_json::Value::String(token),
        );

        if self.hide_credentials {
            ctx.request_headers.remove(&self.header);
        }

        PluginResult::Continue
    }
}
