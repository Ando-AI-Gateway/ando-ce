use crate::plugin::Plugin;
use std::collections::HashMap;
use std::sync::Arc;

/// Thread-safe plugin registry.
///
/// v2 design: Built once at startup, immutable thereafter.
/// Worker cores receive a shared Arc<PluginRegistry>.
pub struct PluginRegistry {
    plugins: HashMap<String, Arc<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin factory.
    pub fn register(&mut self, plugin: Arc<dyn Plugin>) {
        let name = plugin.name().to_string();
        tracing::info!(plugin = %name, "Registered plugin");
        self.plugins.insert(name, plugin);
    }

    /// Get a plugin factory by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Plugin>> {
        self.plugins.get(name)
    }

    /// List all registered plugin names.
    pub fn list(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{Phase, PluginContext, PluginInstance, PluginResult};

    struct MockPlugin {
        name: String,
    }

    impl crate::plugin::Plugin for MockPlugin {
        fn name(&self) -> &str {
            &self.name
        }
        fn priority(&self) -> i32 {
            100
        }
        fn phases(&self) -> &[Phase] {
            &[Phase::Access]
        }
        fn configure(&self, _: &serde_json::Value) -> anyhow::Result<Box<dyn PluginInstance>> {
            let name = self.name.clone();
            struct MockInst(String);
            impl PluginInstance for MockInst {
                fn name(&self) -> &str {
                    &self.0
                }
                fn access(&self, _ctx: &mut PluginContext) -> PluginResult {
                    PluginResult::Continue
                }
            }
            Ok(Box::new(MockInst(name)))
        }
    }

    #[test]
    fn test_empty_registry() {
        let reg = PluginRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_register_and_get() {
        let mut reg = PluginRegistry::new();
        reg.register(Arc::new(MockPlugin {
            name: "key-auth".into(),
        }));
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
        assert!(reg.get("key-auth").is_some());
        assert_eq!(reg.get("key-auth").unwrap().name(), "key-auth");
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_register_multiple() {
        let mut reg = PluginRegistry::new();
        for name in &[
            "key-auth",
            "rate-limiting",
            "ip-restriction",
            "cors",
            "jwt-auth",
        ] {
            reg.register(Arc::new(MockPlugin {
                name: name.to_string(),
            }));
        }
        assert_eq!(reg.len(), 5);
        let names = reg.list();
        assert!(names.contains(&"key-auth"));
        assert!(names.contains(&"cors"));
        assert!(names.contains(&"jwt-auth"));
    }

    #[test]
    fn test_register_overwrite() {
        let mut reg = PluginRegistry::new();
        reg.register(Arc::new(MockPlugin {
            name: "plugin-a".into(),
        }));
        reg.register(Arc::new(MockPlugin {
            name: "plugin-a".into(),
        }));
        // Last write wins; len must still be 1
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_default_is_empty() {
        let reg = PluginRegistry::default();
        assert!(reg.is_empty());
    }

    #[test]
    fn test_configure_via_registry() {
        let mut reg = PluginRegistry::new();
        reg.register(Arc::new(MockPlugin {
            name: "key-auth".into(),
        }));
        let plugin = reg.get("key-auth").unwrap();
        let inst = plugin.configure(&serde_json::json!({})).unwrap();
        assert_eq!(inst.name(), "key-auth");
    }
}
