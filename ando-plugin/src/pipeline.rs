use crate::plugin::{Phase, PluginContext, PluginInstance, PluginResult};
use std::sync::Arc;

/// Pre-built plugin pipeline for a route.
///
/// v2 design: Plugins are sorted by priority at build time.
/// Execution is a simple linear scan — no virtual dispatch overhead
/// beyond the trait object call (which the compiler can devirtualize
/// for common plugin combinations).
pub struct PluginPipeline {
    /// Plugins sorted by phase, then by priority (descending).
    rewrite: Vec<Arc<dyn PluginInstance>>,
    access: Vec<Arc<dyn PluginInstance>>,
    before_proxy: Vec<Arc<dyn PluginInstance>>,
    header_filter: Vec<Arc<dyn PluginInstance>>,
    _body_filter: Vec<Arc<dyn PluginInstance>>,
    log: Vec<Arc<dyn PluginInstance>>,

    /// Pre-computed flags for O(1) phase-presence checks.
    has_rewrite: bool,
    has_access: bool,
    has_before_proxy: bool,
    has_header_filter: bool,
    has_body_filter: bool,
    has_log: bool,

    /// Whether any auth plugin is present (for consumer injection).
    has_auth: bool,
}

impl PluginPipeline {
    /// Build a pipeline from a list of plugin instances.
    pub fn build(instances: Vec<Arc<dyn PluginInstance>>, has_auth: bool) -> Self {
        let mut rewrite = Vec::new();
        let mut access = Vec::new();
        let mut before_proxy = Vec::new();
        let mut header_filter = Vec::new();
        let mut body_filter = Vec::new();
        let mut log = Vec::new();

        // For now, add all instances to all phase vectors.
        // In a production system, we'd have phase metadata per instance.
        // The trait methods have default no-op impls, so calling them is cheap.
        for inst in &instances {
            rewrite.push(Arc::clone(inst));
            access.push(Arc::clone(inst));
            before_proxy.push(Arc::clone(inst));
            header_filter.push(Arc::clone(inst));
            body_filter.push(Arc::clone(inst));
            log.push(Arc::clone(inst));
        }

        // Sort by priority (descending — higher priority first)
        let sort_fn = |a: &Arc<dyn PluginInstance>, b: &Arc<dyn PluginInstance>| {
            b.priority().cmp(&a.priority())
        };
        rewrite.sort_by(sort_fn);
        access.sort_by(sort_fn);
        before_proxy.sort_by(sort_fn);
        header_filter.sort_by(sort_fn);
        body_filter.sort_by(sort_fn);
        log.sort_by(sort_fn);

        Self {
            has_rewrite: !rewrite.is_empty(),
            has_access: !access.is_empty(),
            has_before_proxy: !before_proxy.is_empty(),
            has_header_filter: !header_filter.is_empty(),
            has_body_filter: !body_filter.is_empty(),
            has_log: !log.is_empty(),
            rewrite,
            access,
            before_proxy,
            header_filter,
            _body_filter: body_filter,
            log,
            has_auth,
        }
    }

    /// Execute a specific phase. Returns early on short-circuit.
    #[inline]
    pub fn execute_phase(&self, phase: Phase, ctx: &mut PluginContext) -> PluginResult {
        let plugins = match phase {
            Phase::Rewrite => {
                if !self.has_rewrite {
                    return PluginResult::Continue;
                }
                &self.rewrite
            }
            Phase::Access => {
                if !self.has_access {
                    return PluginResult::Continue;
                }
                &self.access
            }
            Phase::BeforeProxy => {
                if !self.has_before_proxy {
                    return PluginResult::Continue;
                }
                &self.before_proxy
            }
            Phase::HeaderFilter => {
                if !self.has_header_filter {
                    return PluginResult::Continue;
                }
                &self.header_filter
            }
            Phase::BodyFilter | Phase::Log => return PluginResult::Continue,
        };

        for plugin in plugins {
            let result = match phase {
                Phase::Rewrite => plugin.rewrite(ctx),
                Phase::Access => plugin.access(ctx),
                Phase::BeforeProxy => plugin.before_proxy(ctx),
                Phase::HeaderFilter => plugin.header_filter(ctx),
                _ => PluginResult::Continue,
            };

            match result {
                PluginResult::Continue => continue,
                PluginResult::Response { .. } => return result,
            }
        }

        PluginResult::Continue
    }

    /// Execute the log phase (all plugins, fire-and-forget).
    #[inline]
    pub fn execute_log(&self, ctx: &PluginContext) {
        if !self.has_log {
            return;
        }
        for plugin in &self.log {
            plugin.log(ctx);
        }
    }

    /// Check if this pipeline has auth plugins.
    #[inline]
    pub fn has_auth_plugins(&self) -> bool {
        self.has_auth
    }

    /// Check if a given phase has any plugins.
    #[inline]
    pub fn has_phase(&self, phase: Phase) -> bool {
        match phase {
            Phase::Rewrite => self.has_rewrite,
            Phase::Access => self.has_access,
            Phase::BeforeProxy => self.has_before_proxy,
            Phase::HeaderFilter => self.has_header_filter,
            Phase::BodyFilter => self.has_body_filter,
            Phase::Log => self.has_log,
        }
    }

    /// Total number of plugin instances.
    pub fn len(&self) -> usize {
        self.access.len() // representative — all phases have same instances for now
    }

    pub fn is_empty(&self) -> bool {
        self.access.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{PluginContext, PluginInstance, PluginResult};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn make_ctx() -> PluginContext {
        PluginContext::new(
            "r1".into(),
            "127.0.0.1".into(),
            "GET".into(),
            "/test".into(),
            HashMap::new(),
        )
    }

    struct PassPlugin;
    impl PluginInstance for PassPlugin {
        fn name(&self) -> &str {
            "pass"
        }
        fn access(&self, _ctx: &mut PluginContext) -> PluginResult {
            PluginResult::Continue
        }
    }

    struct BlockPlugin {
        status: u16,
    }
    impl PluginInstance for BlockPlugin {
        fn name(&self) -> &str {
            "block"
        }
        fn priority(&self) -> i32 {
            10
        }
        fn access(&self, _ctx: &mut PluginContext) -> PluginResult {
            PluginResult::Response {
                status: self.status,
                headers: vec![],
                body: Some(b"blocked".to_vec()),
            }
        }
    }

    struct SetConsumerPlugin;
    impl PluginInstance for SetConsumerPlugin {
        fn name(&self) -> &str {
            "set-consumer"
        }
        fn rewrite(&self, ctx: &mut PluginContext) -> PluginResult {
            ctx.consumer = Some("alice".into());
            PluginResult::Continue
        }
    }

    #[test]
    fn test_empty_pipeline_continue() {
        let pipeline = PluginPipeline::build(vec![], false);
        let mut ctx = make_ctx();
        let result = pipeline.execute_phase(Phase::Access, &mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert!(pipeline.is_empty());
        assert!(!pipeline.has_auth_plugins());
    }

    #[test]
    fn test_pass_plugin_continues() {
        let plugin: Arc<dyn PluginInstance> = Arc::new(PassPlugin);
        let pipeline = PluginPipeline::build(vec![plugin], false);
        let mut ctx = make_ctx();
        let result = pipeline.execute_phase(Phase::Access, &mut ctx);
        assert!(matches!(result, PluginResult::Continue));
    }

    #[test]
    fn test_block_plugin_short_circuits() {
        let plugin: Arc<dyn PluginInstance> = Arc::new(BlockPlugin { status: 403 });
        let pipeline = PluginPipeline::build(vec![plugin], false);
        let mut ctx = make_ctx();
        let result = pipeline.execute_phase(Phase::Access, &mut ctx);
        if let PluginResult::Response { status, body, .. } = result {
            assert_eq!(status, 403);
            assert_eq!(body.unwrap(), b"blocked");
        } else {
            panic!("Expected Response from block plugin");
        }
    }

    #[test]
    fn test_rewrite_phase_modifies_context() {
        let plugin: Arc<dyn PluginInstance> = Arc::new(SetConsumerPlugin);
        let pipeline = PluginPipeline::build(vec![plugin], false);
        let mut ctx = make_ctx();
        let result = pipeline.execute_phase(Phase::Rewrite, &mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.consumer.as_deref(), Some("alice"));
    }

    #[test]
    fn test_has_phase_with_plugins() {
        let plugin: Arc<dyn PluginInstance> = Arc::new(PassPlugin);
        let pipeline = PluginPipeline::build(vec![plugin], true);
        assert!(pipeline.has_phase(Phase::Access));
        assert!(pipeline.has_phase(Phase::Rewrite));
        assert!(pipeline.has_auth_plugins());
        assert_eq!(pipeline.len(), 1);
    }

    #[test]
    fn test_empty_pipeline_no_phases() {
        let pipeline = PluginPipeline::build(vec![], false);
        assert!(!pipeline.has_phase(Phase::Access));
        assert!(!pipeline.has_phase(Phase::Rewrite));
        assert!(!pipeline.has_phase(Phase::HeaderFilter));
        assert!(!pipeline.has_phase(Phase::Log));
    }

    // ── Priority ordering: higher priority runs first ─────────────

    /// A plugin that appends its name to a shared log vec via context vars.
    struct OrderPlugin {
        label: String,
        prio: i32,
    }
    impl PluginInstance for OrderPlugin {
        fn name(&self) -> &str {
            &self.label
        }
        fn priority(&self) -> i32 {
            self.prio
        }
        fn access(&self, ctx: &mut PluginContext) -> PluginResult {
            let log = ctx
                .vars
                .entry("_order".to_string())
                .or_insert_with(|| serde_json::Value::Array(vec![]));
            if let serde_json::Value::Array(arr) = log {
                arr.push(serde_json::Value::String(self.label.clone()));
            }
            PluginResult::Continue
        }
    }

    #[test]
    fn plugins_execute_in_priority_order_descending() {
        let a: Arc<dyn PluginInstance> = Arc::new(OrderPlugin {
            label: "low".into(),
            prio: 100,
        });
        let b: Arc<dyn PluginInstance> = Arc::new(OrderPlugin {
            label: "high".into(),
            prio: 3000,
        });
        let c: Arc<dyn PluginInstance> = Arc::new(OrderPlugin {
            label: "mid".into(),
            prio: 1000,
        });

        let pipeline = PluginPipeline::build(vec![a, b, c], false);
        let mut ctx = make_ctx();
        pipeline.execute_phase(Phase::Access, &mut ctx);

        let order = ctx
            .vars
            .get("_order")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
        assert_eq!(
            order,
            vec!["high", "mid", "low"],
            "plugins must execute in descending priority order"
        );
    }

    // ── Block plugin short-circuits: later plugins DO NOT run ─────

    #[test]
    fn block_plugin_prevents_later_plugins_from_running() {
        let first: Arc<dyn PluginInstance> = Arc::new(OrderPlugin {
            label: "first".into(),
            prio: 3000,
        });
        let _blocker: Arc<dyn PluginInstance> = Arc::new(BlockPlugin { status: 403 });
        // BlockPlugin has priority 10 — but we'll bump it
        struct HighBlockPlugin;
        impl PluginInstance for HighBlockPlugin {
            fn name(&self) -> &str {
                "high-block"
            }
            fn priority(&self) -> i32 {
                2000
            }
            fn access(&self, _ctx: &mut PluginContext) -> PluginResult {
                PluginResult::Response {
                    status: 403,
                    headers: vec![],
                    body: None,
                }
            }
        }
        let high_blocker: Arc<dyn PluginInstance> = Arc::new(HighBlockPlugin);
        let last: Arc<dyn PluginInstance> = Arc::new(OrderPlugin {
            label: "last".into(),
            prio: 100,
        });

        let pipeline = PluginPipeline::build(vec![first, high_blocker, last], false);
        let mut ctx = make_ctx();
        let result = pipeline.execute_phase(Phase::Access, &mut ctx);

        assert!(matches!(result, PluginResult::Response { status: 403, .. }));
        // "first" (prio 3000) should have run, "last" (prio 100) should NOT
        let order = ctx
            .vars
            .get("_order")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
        assert_eq!(
            order,
            vec!["first"],
            "only plugins before the blocker should have run"
        );
    }

    // ── Multiple phases execute independently ─────────────────────

    #[test]
    fn different_phases_are_independent() {
        struct RewriteOnlyPlugin;
        impl PluginInstance for RewriteOnlyPlugin {
            fn name(&self) -> &str {
                "rw-only"
            }
            fn rewrite(&self, ctx: &mut PluginContext) -> PluginResult {
                ctx.consumer = Some("rewrite-ran".into());
                PluginResult::Continue
            }
        }
        let plugin: Arc<dyn PluginInstance> = Arc::new(RewriteOnlyPlugin);
        let pipeline = PluginPipeline::build(vec![plugin], false);
        let mut ctx = make_ctx();

        // Access phase: default no-op implementation should Continue
        let result = pipeline.execute_phase(Phase::Access, &mut ctx);
        assert!(matches!(result, PluginResult::Continue));
        // consumer not set by access phase
        assert!(ctx.consumer.is_none());

        // Rewrite phase: should set consumer
        pipeline.execute_phase(Phase::Rewrite, &mut ctx);
        assert_eq!(ctx.consumer.as_deref(), Some("rewrite-ran"));
    }
}
