use ando_plugin::pipeline::PluginPipeline;
use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginInstance, PluginResult};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

// =============================================================================
// Test Plugin Implementations
// =============================================================================

struct PassthroughPlugin {
    name: String,
    priority: i32,
    phases: Vec<Phase>,
}

impl PassthroughPlugin {
    fn new(name: &str, priority: i32, phases: Vec<Phase>) -> Self {
        Self {
            name: name.to_string(),
            priority,
            phases,
        }
    }
}

#[async_trait]
impl Plugin for PassthroughPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn phases(&self) -> Vec<Phase> {
        self.phases.clone()
    }

    async fn execute(
        &self,
        _phase: Phase,
        ctx: &mut PluginContext,
        _config: &Value,
    ) -> PluginResult {
        ctx.set_var(
            format!("executed_{}", self.name),
            serde_json::json!(true),
        );
        PluginResult::Continue
    }
}

struct RejectPlugin {
    name: String,
    priority: i32,
    status: u16,
}

impl RejectPlugin {
    fn new(name: &str, priority: i32, status: u16) -> Self {
        Self {
            name: name.to_string(),
            priority,
            status,
        }
    }
}

#[async_trait]
impl Plugin for RejectPlugin {
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
        PluginResult::Response {
            status: self.status,
            headers: HashMap::from([
                ("content-type".to_string(), "application/json".to_string()),
            ]),
            body: Some(b"rejected".to_vec()),
        }
    }
}

struct ErrorPlugin {
    name: String,
    priority: i32,
}

#[async_trait]
impl Plugin for ErrorPlugin {
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
        PluginResult::Error("test error".to_string())
    }
}

fn make_ctx() -> PluginContext {
    PluginContext::new(
        "GET".to_string(),
        "/api/test".to_string(),
        HashMap::new(),
        "127.0.0.1".to_string(),
        "r1".to_string(),
    )
}

fn make_instance(plugin: Arc<dyn Plugin>, config: Value) -> PluginInstance {
    PluginInstance::new(plugin, config)
}

// =============================================================================
// Pipeline Construction Tests
// =============================================================================

#[test]
fn test_pipeline_empty() {
    let pipeline = PluginPipeline::new(vec![]);
    assert_eq!(pipeline.plugin_count(), 0);
}

#[test]
fn test_pipeline_single_plugin() {
    let plugin = Arc::new(PassthroughPlugin::new("test", 100, vec![Phase::Access]));
    let instances = vec![make_instance(plugin, serde_json::json!({}))];
    let pipeline = PluginPipeline::new(instances);
    assert_eq!(pipeline.plugin_count(), 1);
}

#[test]
fn test_pipeline_multi_phase_plugin() {
    let plugin = Arc::new(PassthroughPlugin::new(
        "multi",
        100,
        vec![Phase::Rewrite, Phase::Access, Phase::HeaderFilter],
    ));
    let instances = vec![make_instance(plugin, serde_json::json!({}))];
    let pipeline = PluginPipeline::new(instances);
    // One plugin instance per phase = 3
    assert_eq!(pipeline.plugin_count(), 3);
}

// =============================================================================
// Pipeline Execution Tests
// =============================================================================

#[tokio::test]
async fn test_pipeline_execute_empty_phase() {
    let pipeline = PluginPipeline::new(vec![]);
    let mut ctx = make_ctx();

    let result = pipeline.execute_phase(Phase::Access, &mut ctx).await;
    assert!(matches!(result, PluginResult::Continue));
}

#[tokio::test]
async fn test_pipeline_execute_passthrough() {
    let plugin = Arc::new(PassthroughPlugin::new("pass", 100, vec![Phase::Access]));
    let instances = vec![make_instance(plugin, serde_json::json!({}))];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    let result = pipeline.execute_phase(Phase::Access, &mut ctx).await;
    assert!(matches!(result, PluginResult::Continue));
    assert_eq!(ctx.get_var("executed_pass"), Some(&serde_json::json!(true)));
}

#[tokio::test]
async fn test_pipeline_execute_multiple_plugins() {
    let p1 = Arc::new(PassthroughPlugin::new("first", 200, vec![Phase::Access]));
    let p2 = Arc::new(PassthroughPlugin::new("second", 100, vec![Phase::Access]));
    let instances = vec![
        make_instance(p1, serde_json::json!({})),
        make_instance(p2, serde_json::json!({})),
    ];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    let result = pipeline.execute_phase(Phase::Access, &mut ctx).await;
    assert!(matches!(result, PluginResult::Continue));
    // Both plugins should have executed
    assert_eq!(ctx.get_var("executed_first"), Some(&serde_json::json!(true)));
    assert_eq!(ctx.get_var("executed_second"), Some(&serde_json::json!(true)));
}

#[tokio::test]
async fn test_pipeline_short_circuit_on_response() {
    let p1 = Arc::new(PassthroughPlugin::new("first", 200, vec![Phase::Access]));
    let reject = Arc::new(RejectPlugin::new("reject", 150, 403));
    let p3 = Arc::new(PassthroughPlugin::new("third", 100, vec![Phase::Access]));
    let instances = vec![
        make_instance(p1, serde_json::json!({})),
        make_instance(reject, serde_json::json!({})),
        make_instance(p3, serde_json::json!({})),
    ];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    let result = pipeline.execute_phase(Phase::Access, &mut ctx).await;
    match result {
        PluginResult::Response { status, .. } => assert_eq!(status, 403),
        _ => panic!("Expected Response result"),
    }
    // First plugin executed (higher priority)
    assert_eq!(ctx.get_var("executed_first"), Some(&serde_json::json!(true)));
    // Third plugin should NOT have executed (short-circuited)
    assert!(ctx.get_var("executed_third").is_none());
}

#[tokio::test]
async fn test_pipeline_short_circuit_on_error() {
    let p1 = Arc::new(PassthroughPlugin::new("first", 200, vec![Phase::Access]));
    let err = Arc::new(ErrorPlugin {
        name: "error".to_string(),
        priority: 150,
    });
    let p3 = Arc::new(PassthroughPlugin::new("third", 100, vec![Phase::Access]));
    let instances = vec![
        make_instance(p1, serde_json::json!({})),
        make_instance(err, serde_json::json!({})),
        make_instance(p3, serde_json::json!({})),
    ];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    let result = pipeline.execute_phase(Phase::Access, &mut ctx).await;
    match result {
        PluginResult::Error(msg) => assert_eq!(msg, "test error"),
        _ => panic!("Expected Error result"),
    }
    assert!(ctx.get_var("executed_third").is_none());
}

// =============================================================================
// Request/Response Phase Execution Tests
// =============================================================================

#[tokio::test]
async fn test_execute_request_phases() {
    let rewrite_plugin = Arc::new(PassthroughPlugin::new("rewrite", 100, vec![Phase::Rewrite]));
    let access_plugin = Arc::new(PassthroughPlugin::new("access", 100, vec![Phase::Access]));
    let before_proxy = Arc::new(PassthroughPlugin::new("before_proxy", 100, vec![Phase::BeforeProxy]));

    let instances = vec![
        make_instance(rewrite_plugin, serde_json::json!({})),
        make_instance(access_plugin, serde_json::json!({})),
        make_instance(before_proxy, serde_json::json!({})),
    ];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    let result = pipeline.execute_request_phases(&mut ctx).await;
    assert!(matches!(result, PluginResult::Continue));
    assert_eq!(ctx.get_var("executed_rewrite"), Some(&serde_json::json!(true)));
    assert_eq!(ctx.get_var("executed_access"), Some(&serde_json::json!(true)));
    assert_eq!(ctx.get_var("executed_before_proxy"), Some(&serde_json::json!(true)));
}

#[tokio::test]
async fn test_execute_request_phases_short_circuit() {
    let rewrite_plugin = Arc::new(PassthroughPlugin::new("rewrite", 100, vec![Phase::Rewrite]));
    let reject = Arc::new(RejectPlugin::new("reject", 100, 401));
    // This is in AccessPhase by default
    let before_proxy = Arc::new(PassthroughPlugin::new("before_proxy", 100, vec![Phase::BeforeProxy]));

    let instances = vec![
        make_instance(rewrite_plugin, serde_json::json!({})),
        make_instance(reject, serde_json::json!({})),
        make_instance(before_proxy, serde_json::json!({})),
    ];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    let result = pipeline.execute_request_phases(&mut ctx).await;
    match result {
        PluginResult::Response { status, .. } => assert_eq!(status, 401),
        _ => panic!("Expected Response"),
    }
    // Rewrite ran, but BeforeProxy should not have
    assert_eq!(ctx.get_var("executed_rewrite"), Some(&serde_json::json!(true)));
    assert!(ctx.get_var("executed_before_proxy").is_none());
}

#[tokio::test]
async fn test_execute_response_phases() {
    let header_filter = Arc::new(PassthroughPlugin::new("header_filter", 100, vec![Phase::HeaderFilter]));
    let body_filter = Arc::new(PassthroughPlugin::new("body_filter", 100, vec![Phase::BodyFilter]));

    let instances = vec![
        make_instance(header_filter, serde_json::json!({})),
        make_instance(body_filter, serde_json::json!({})),
    ];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    let result = pipeline.execute_response_phases(&mut ctx).await;
    assert!(matches!(result, PluginResult::Continue));
    assert_eq!(ctx.get_var("executed_header_filter"), Some(&serde_json::json!(true)));
    assert_eq!(ctx.get_var("executed_body_filter"), Some(&serde_json::json!(true)));
}

#[tokio::test]
async fn test_execute_log_phase() {
    let log_plugin = Arc::new(PassthroughPlugin::new("logger", 100, vec![Phase::Log]));

    let instances = vec![make_instance(log_plugin, serde_json::json!({}))];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    // Log phase doesn't return a result
    pipeline.execute_log_phase(&mut ctx).await;
    assert_eq!(ctx.get_var("executed_logger"), Some(&serde_json::json!(true)));
}

#[tokio::test]
async fn test_execute_log_phase_error_is_nonfatal() {
    let err_plugin = Arc::new(ErrorPlugin {
        name: "error-logger".to_string(),
        priority: 100,
    });
    // ErrorPlugin defaults to Access phase, we need Log phase
    let log_plugin = Arc::new(PassthroughPlugin::new("post-error", 50, vec![Phase::Log]));

    let instances = vec![
        make_instance(err_plugin, serde_json::json!({})),
        make_instance(log_plugin, serde_json::json!({})),
    ];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    // Error plugin is in Access phase, not Log phase, so log phase should succeed
    pipeline.execute_log_phase(&mut ctx).await;
    assert_eq!(ctx.get_var("executed_post-error"), Some(&serde_json::json!(true)));
}

// =============================================================================
// Priority Ordering Tests
// =============================================================================

#[tokio::test]
async fn test_plugins_execute_in_priority_order() {
    // Higher priority executes first
    struct OrderTracker {
        name: String,
        priority: i32,
    }

    #[async_trait]
    impl Plugin for OrderTracker {
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
            ctx: &mut PluginContext,
            _config: &Value,
        ) -> PluginResult {
            let order: Vec<String> = ctx
                .get_var("execution_order")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let mut order = order;
            order.push(self.name.clone());
            ctx.set_var("execution_order".to_string(), serde_json::json!(order));
            PluginResult::Continue
        }
    }

    let p_low = Arc::new(OrderTracker {
        name: "low".to_string(),
        priority: 100,
    });
    let p_high = Arc::new(OrderTracker {
        name: "high".to_string(),
        priority: 300,
    });
    let p_mid = Arc::new(OrderTracker {
        name: "mid".to_string(),
        priority: 200,
    });

    let instances = vec![
        make_instance(p_low, serde_json::json!({})),
        make_instance(p_high, serde_json::json!({})),
        make_instance(p_mid, serde_json::json!({})),
    ];
    let pipeline = PluginPipeline::new(instances);
    let mut ctx = make_ctx();

    pipeline.execute_phase(Phase::Access, &mut ctx).await;

    let order: Vec<String> = serde_json::from_value(
        ctx.get_var("execution_order").unwrap().clone(),
    )
    .unwrap();

    assert_eq!(order, vec!["high", "mid", "low"]);
}
