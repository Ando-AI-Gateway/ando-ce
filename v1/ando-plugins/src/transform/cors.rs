use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// CORS (Cross-Origin Resource Sharing) plugin.
///
/// Configuration:
/// ```json
/// {
///   "allow_origins": "*",
///   "allow_methods": "GET, POST, PUT, DELETE, OPTIONS",
///   "allow_headers": "Content-Type, Authorization",
///   "expose_headers": "X-RateLimit-Limit",
///   "max_age": 3600,
///   "allow_credential": false
/// }
/// ```
pub struct CorsPlugin;

impl CorsPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for CorsPlugin {
    fn name(&self) -> &str {
        "cors"
    }

    fn priority(&self) -> i32 {
        4000
    }

    fn phases(&self) -> Vec<Phase> {
        vec![Phase::Rewrite, Phase::HeaderFilter]
    }

    async fn execute(
        &self,
        phase: Phase,
        ctx: &mut PluginContext,
        config: &Value,
    ) -> PluginResult {
        let allow_origins = config
            .get("allow_origins")
            .and_then(|v| v.as_str())
            .unwrap_or("*");

        let allow_methods = config
            .get("allow_methods")
            .and_then(|v| v.as_str())
            .unwrap_or("GET, POST, PUT, DELETE, PATCH, OPTIONS");

        let allow_headers = config
            .get("allow_headers")
            .and_then(|v| v.as_str())
            .unwrap_or("Content-Type, Authorization, X-Requested-With");

        let expose_headers = config
            .get("expose_headers")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let max_age = config
            .get("max_age")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        let allow_credential = config
            .get("allow_credential")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        match phase {
            Phase::Rewrite => {
                // Handle preflight OPTIONS
                if ctx.request_method == "OPTIONS" {
                    let mut headers = HashMap::new();
                    headers.insert(
                        "Access-Control-Allow-Origin".to_string(),
                        allow_origins.to_string(),
                    );
                    headers.insert(
                        "Access-Control-Allow-Methods".to_string(),
                        allow_methods.to_string(),
                    );
                    headers.insert(
                        "Access-Control-Allow-Headers".to_string(),
                        allow_headers.to_string(),
                    );
                    headers.insert(
                        "Access-Control-Max-Age".to_string(),
                        max_age.to_string(),
                    );
                    if allow_credential {
                        headers.insert(
                            "Access-Control-Allow-Credentials".to_string(),
                            "true".to_string(),
                        );
                    }

                    return PluginResult::Response {
                        status: 204,
                        headers,
                        body: None,
                    };
                }
                PluginResult::Continue
            }
            Phase::HeaderFilter => {
                ctx.set_response_header(
                    "Access-Control-Allow-Origin".to_string(),
                    allow_origins.to_string(),
                );
                if !expose_headers.is_empty() {
                    ctx.set_response_header(
                        "Access-Control-Expose-Headers".to_string(),
                        expose_headers.to_string(),
                    );
                }
                if allow_credential {
                    ctx.set_response_header(
                        "Access-Control-Allow-Credentials".to_string(),
                        "true".to_string(),
                    );
                }
                PluginResult::Continue
            }
            _ => PluginResult::Continue,
        }
    }
}
