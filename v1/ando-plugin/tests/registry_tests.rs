use ando_plugin::registry::PluginRegistry;
use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

// =============================================================================
// Test Plugin Implementation
// =============================================================================

struct TestPlugin {
    name: String,
    priority: i32,
}

impl TestPlugin {
    fn new(name: &str, priority: i32) -> Self {
        Self {
            name: name.to_string(),
            priority,
        }
    }
}

#[async_trait]
impl Plugin for TestPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn phases(&self) -> Vec<Phase> {
        vec![Phase::Access]
    }

    async fn execute(
        &self,
        _phase: Phase,
        _ctx: &mut PluginContext,
        _config: &Value,
    ) -> PluginResult {
        PluginResult::Continue
    }
}

// =============================================================================
// Registry Tests
// =============================================================================

#[test]
fn test_registry_new() {
    let registry = PluginRegistry::new();
    assert_eq!(registry.count(), 0);
    assert!(registry.list().is_empty());
}

#[test]
fn test_registry_default() {
    let registry = PluginRegistry::default();
    assert_eq!(registry.count(), 0);
}

#[test]
fn test_registry_register_plugin() {
    let registry = PluginRegistry::new();
    registry.register(Arc::new(TestPlugin::new("test-plugin", 100)));
    assert_eq!(registry.count(), 1);
    assert!(registry.contains("test-plugin"));
}

#[test]
fn test_registry_get_plugin() {
    let registry = PluginRegistry::new();
    registry.register(Arc::new(TestPlugin::new("my-plugin", 200)));

    let plugin = registry.get("my-plugin");
    assert!(plugin.is_some());
    assert_eq!(plugin.unwrap().name(), "my-plugin");
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_registry_contains() {
    let registry = PluginRegistry::new();
    registry.register(Arc::new(TestPlugin::new("existing", 100)));

    assert!(registry.contains("existing"));
    assert!(!registry.contains("missing"));
}

#[test]
fn test_registry_list_plugins() {
    let registry = PluginRegistry::new();
    registry.register(Arc::new(TestPlugin::new("alpha", 100)));
    registry.register(Arc::new(TestPlugin::new("beta", 200)));
    registry.register(Arc::new(TestPlugin::new("gamma", 300)));

    let list = registry.list();
    assert_eq!(list.len(), 3);
    assert!(list.contains(&"alpha".to_string()));
    assert!(list.contains(&"beta".to_string()));
    assert!(list.contains(&"gamma".to_string()));
}

#[test]
fn test_registry_multiple_plugins() {
    let registry = PluginRegistry::new();
    for i in 0..10 {
        registry.register(Arc::new(TestPlugin::new(&format!("plugin-{}", i), i)));
    }
    assert_eq!(registry.count(), 10);
}

#[test]
fn test_registry_overwrite_plugin() {
    let registry = PluginRegistry::new();
    registry.register(Arc::new(TestPlugin::new("same-name", 100)));
    registry.register(Arc::new(TestPlugin::new("same-name", 200)));

    assert_eq!(registry.count(), 1);
    let plugin = registry.get("same-name").unwrap();
    assert_eq!(plugin.priority(), 200);
}

#[test]
fn test_registry_concurrent_access() {
    use std::thread;

    let registry = Arc::new(PluginRegistry::new());

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let reg = Arc::clone(&registry);
            thread::spawn(move || {
                reg.register(Arc::new(TestPlugin::new(&format!("plugin-{}", i), i)));
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(registry.count(), 10);
}
