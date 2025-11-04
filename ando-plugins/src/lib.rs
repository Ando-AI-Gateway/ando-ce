pub mod auth;
pub mod traffic;
pub mod transform;

use ando_plugin::registry::PluginRegistry;
use std::sync::Arc;

/// Register all built-in Community Edition plugins.
pub fn register_all(registry: &mut PluginRegistry) {
    // Auth plugins
    registry.register(Arc::new(auth::key_auth::KeyAuthPlugin));
    registry.register(Arc::new(auth::jwt_auth::JwtAuthPlugin));
    registry.register(Arc::new(auth::basic_auth::BasicAuthPlugin));

    // Traffic control plugins
    registry.register(Arc::new(traffic::ip_restriction::IpRestrictionPlugin));
    registry.register(Arc::new(traffic::rate_limiting::RateLimitingPlugin));

    // Transform plugins
    registry.register(Arc::new(transform::cors::CorsPlugin));
}
