use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use serde::Deserialize;

/// CORS (Cross-Origin Resource Sharing) plugin.
///
/// Handles preflight OPTIONS requests and adds CORS headers to responses.
pub struct CorsPlugin;

#[derive(Debug, Deserialize, Clone)]
struct CorsConfig {
    /// Allowed origins (default: "*").
    #[serde(default = "default_origins")]
    allow_origins: String,
    /// Allowed methods.
    #[serde(default = "default_methods")]
    allow_methods: String,
    /// Allowed headers.
    #[serde(default = "default_headers")]
    allow_headers: String,
    /// Exposed headers.
    #[serde(default)]
    expose_headers: String,
    /// Max age for preflight cache (seconds).
    #[serde(default = "default_max_age")]
    max_age: u64,
    /// Allow credentials.
    #[serde(default)]
    allow_credential: bool,
}

fn default_origins() -> String {
    "*".to_string()
}

fn default_methods() -> String {
    "GET,POST,PUT,DELETE,PATCH,HEAD,OPTIONS".to_string()
}

fn default_headers() -> String {
    "Content-Type,Authorization,X-Requested-With".to_string()
}

fn default_max_age() -> u64 {
    5 // seconds
}

struct CorsInstance {
    allow_origins: String,
    allow_methods: String,
    allow_headers: String,
    expose_headers: String,
    max_age: String,
    allow_credential: bool,
}

impl Plugin for CorsPlugin {
    fn name(&self) -> &str {
        "cors"
    }

    fn priority(&self) -> i32 {
        4000 // APISIX default priority for cors
    }

    fn phases(&self) -> &[Phase] {
        &[Phase::Rewrite, Phase::HeaderFilter]
    }

    fn configure(&self, config: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
        let cfg: CorsConfig = serde_json::from_value(config.clone()).unwrap_or(CorsConfig {
            allow_origins: default_origins(),
            allow_methods: default_methods(),
            allow_headers: default_headers(),
            expose_headers: String::new(),
            max_age: default_max_age(),
            allow_credential: false,
        });

        Ok(Box::new(CorsInstance {
            allow_origins: cfg.allow_origins,
            allow_methods: cfg.allow_methods,
            allow_headers: cfg.allow_headers,
            expose_headers: cfg.expose_headers,
            max_age: cfg.max_age.to_string(),
            allow_credential: cfg.allow_credential,
        }))
    }
}

impl PluginInstance for CorsInstance {
    fn name(&self) -> &str {
        "cors"
    }

    fn priority(&self) -> i32 {
        4000
    }

    /// Handle preflight OPTIONS requests.
    fn rewrite(&self, ctx: &mut PluginContext) -> PluginResult {
        if ctx.method == "OPTIONS" {
            let mut headers = vec![
                (
                    "access-control-allow-origin".to_string(),
                    self.allow_origins.clone(),
                ),
                (
                    "access-control-allow-methods".to_string(),
                    self.allow_methods.clone(),
                ),
                (
                    "access-control-allow-headers".to_string(),
                    self.allow_headers.clone(),
                ),
                (
                    "access-control-max-age".to_string(),
                    self.max_age.clone(),
                ),
            ];

            if self.allow_credential {
                headers.push((
                    "access-control-allow-credentials".to_string(),
                    "true".to_string(),
                ));
            }

            if !self.expose_headers.is_empty() {
                headers.push((
                    "access-control-expose-headers".to_string(),
                    self.expose_headers.clone(),
                ));
            }

            return PluginResult::Response {
                status: 204,
                headers,
                body: None,
            };
        }

        PluginResult::Continue
    }

    /// Add CORS headers to all responses.
    fn header_filter(&self, ctx: &mut PluginContext) -> PluginResult {
        ctx.response_headers.insert(
            "access-control-allow-origin".to_string(),
            self.allow_origins.clone(),
        );

        if self.allow_credential {
            ctx.response_headers.insert(
                "access-control-allow-credentials".to_string(),
                "true".to_string(),
            );
        }

        if !self.expose_headers.is_empty() {
            ctx.response_headers.insert(
                "access-control-expose-headers".to_string(),
                self.expose_headers.clone(),
            );
        }

        PluginResult::Continue
    }
}
