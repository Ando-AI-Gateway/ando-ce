pub mod auth;

use ando_plugin::registry::PluginRegistry;
use std::sync::Arc;

/// Register all built-in plugins.
pub fn register_all(registry: &mut PluginRegistry) {
    registry.register(Arc::new(auth::key_auth::KeyAuthPlugin));
}
