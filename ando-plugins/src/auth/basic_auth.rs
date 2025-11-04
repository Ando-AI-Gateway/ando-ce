use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use serde::Deserialize;

/// Basic HTTP authentication plugin.
///
/// Validates credentials from the `Authorization: Basic <base64>` header.
/// Consumer lookup is done via the `username` matching a consumer's basic-auth config.
pub struct BasicAuthPlugin;

#[derive(Debug, Deserialize)]
struct BasicAuthConfig {
    /// Whether to hide the auth header from upstream.
    #[serde(default)]
    hide_credentials: bool,
}

struct BasicAuthInstance {
    hide_credentials: bool,
}

impl Plugin for BasicAuthPlugin {
    fn name(&self) -> &str {
        "basic-auth"
    }

    fn priority(&self) -> i32 {
        2520 // APISIX default priority for basic-auth
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Access]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: BasicAuthConfig = serde_json::from_value(config.clone())
            .unwrap_or(BasicAuthConfig {
                hide_credentials: false,
            });

        Ok(Box::new(BasicAuthInstance {
            hide_credentials: cfg.hide_credentials,
        }))
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
        let auth_header = match ctx.get_header("authorization") {
            Some(val) => val.to_string(),
            None => {
                return PluginResult::Response {
                    status: 401,
                    headers: vec![
                        ("content-type".to_string(), "application/json".to_string()),
                        (
                            "www-authenticate".to_string(),
                            "Basic realm=\"Ando\"".to_string(),
                        ),
                    ],
                    body: Some(
                        br#"{"error":"Missing authorization header","status":401}"#.to_vec(),
                    ),
                };
            }
        };

        // Extract and validate Basic auth format
        let credentials = if let Some(stripped) = auth_header.strip_prefix("Basic ") {
            stripped.to_string()
        } else if let Some(stripped) = auth_header.strip_prefix("basic ") {
            stripped.to_string()
        } else {
            return PluginResult::Response {
                status: 401,
                headers: vec![
                    ("content-type".to_string(), "application/json".to_string()),
                ],
                body: Some(
                    br#"{"error":"Invalid authorization scheme, expected Basic","status":401}"#
                        .to_vec(),
                ),
            };
        };

        // Store the base64-encoded credentials in vars for the proxy to validate.
        // The proxy layer decodes and checks against consumer store.
        ctx.vars.insert(
            "_basic_auth_credentials".to_string(),
            serde_json::Value::String(credentials),
        );

        if self.hide_credentials {
            ctx.request_headers.remove("authorization");
        }

        PluginResult::Continue
    }
}
