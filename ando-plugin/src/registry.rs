use crate::plugin::Plugin;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::info;

/// Global plugin registry.
///
/// Stores all available plugin implementations (both Rust native and Lua).
/// Plugins are registered by name and can be looked up to create instances.
pub struct PluginRegistry {
    plugins: DashMap<String, Arc<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: DashMap::new(),
        }
    }

    /// Register a plugin implementation.
    pub fn register(&self, plugin: Arc<dyn Plugin>) {
        let name = plugin.name().to_string();
        info!(plugin = %name, priority = plugin.priority(), "Registering plugin");
        self.plugins.insert(name, plugin);
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Plugin>> {
        self.plugins.get(name).map(|p| Arc::clone(p.value()))
    }

    /// Check if a plugin exists.
    pub fn contains(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// List all registered plugin names.
    pub fn list(&self) -> Vec<String> {
        self.plugins.iter().map(|r| r.key().clone()).collect()
    }

    /// Get the total number of registered plugins.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
