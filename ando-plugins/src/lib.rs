pub mod auth;
pub mod traffic;

use ando_plugin::registry::PluginRegistry;
use std::sync::Arc;

/// Register all built-in plugins.
pub fn register_all(registry: &mut PluginRegistry) {
    registry.register(Arc::new(auth::key_auth::KeyAuthPlugin));
    registry.register(Arc::new(auth::basic_auth::BasicAuthPlugin));
    registry.register(Arc::new(auth::jwt_auth::JwtAuthPlugin));
    registry.register(Arc::new(traffic::ip_restriction::IpRestrictionPlugin));
    registry.register(Arc::new(traffic::rate_limiting::RateLimitingPlugin));
    registry.register(Arc::new(traffic::cors::CorsPlugin));
    registry.register(Arc::new(traffic::security_headers::SecurityHeadersPlugin));
}
