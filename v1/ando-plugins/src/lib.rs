pub mod auth;
pub mod observability;
pub mod security;
pub mod traffic;
pub mod transform;

use ando_plugin::registry::PluginRegistry;
use std::sync::Arc;

/// Register all built-in plugins with the registry.
pub fn register_all(registry: &PluginRegistry) {
    // Auth plugins
    registry.register(Arc::new(auth::key_auth::KeyAuthPlugin::new()));
    registry.register(Arc::new(auth::jwt_auth::JwtAuthPlugin::new()));
    registry.register(Arc::new(auth::basic_auth::BasicAuthPlugin::new()));

    // Traffic control plugins
    registry.register(Arc::new(traffic::limit_count::LimitCountPlugin::new()));
    registry.register(Arc::new(traffic::limit_req::LimitReqPlugin::new()));

    // Transform plugins
    registry.register(Arc::new(transform::cors::CorsPlugin::new()));
    registry.register(Arc::new(transform::request_transformer::RequestTransformerPlugin::new()));
    registry.register(Arc::new(transform::response_transformer::ResponseTransformerPlugin::new()));

    // Security plugins
    registry.register(Arc::new(security::ip_restriction::IpRestrictionPlugin::new()));

    tracing::info!(count = registry.count(), "Built-in plugins registered");
}
