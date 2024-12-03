use crate::plugin::{Phase, PluginContext, PluginInstance, PluginResult};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, warn};

/// The plugin execution pipeline.
///
/// Executes plugins in priority order for each phase.
/// Short-circuits if any plugin returns a response or error.
///
/// Optimized: stores a phase_mask bitfield for O(1) `has_phase()` checks
/// so callers can skip entire phase callbacks without HashMap lookups.
pub struct PluginPipeline {
    /// Plugins sorted by phase and priority
    phases: HashMap<Phase, Vec<PluginInstance>>,
    /// Bitmask of which phases are populated (1 << phase as u8).
    phase_mask: u8,
}

impl PluginPipeline {
    /// Build a pipeline from a list of plugin instances.
    pub fn new(mut instances: Vec<PluginInstance>) -> Self {
        let mut phases: HashMap<Phase, Vec<PluginInstance>> = HashMap::new();

        // Sort by priority (higher first)
        instances.sort_by(|a, b| b.plugin.priority().cmp(&a.plugin.priority()));

        for instance in instances {
            let plugin_phases = instance.plugin.phases();
            for phase in &plugin_phases {
                let pi = PluginInstance {
                    plugin: Arc::clone(&instance.plugin),
                    config: instance.config.clone(),
                    name: instance.name.clone(),
                };
                phases.entry(*phase).or_default().push(pi);
            }
        }

        let mut phase_mask: u8 = 0;
        for phase in phases.keys() {
            phase_mask |= 1 << (*phase as u8);
        }

        Self { phases, phase_mask }
    }

    /// O(1) check whether this pipeline has any plugins for the given phase.
    #[inline(always)]
    pub fn has_phase(&self, phase: Phase) -> bool {
        self.phase_mask & (1 << (phase as u8)) != 0
    }

    /// Execute all plugins for a given phase.
    ///
    /// Returns `PluginResult::Continue` if all plugins pass,
    /// or short-circuits with a response/error.
    #[inline]
    pub async fn execute_phase(
        &self,
        phase: Phase,
        ctx: &mut PluginContext,
    ) -> PluginResult {
        let Some(plugins) = self.phases.get(&phase) else {
            return PluginResult::Continue;
        };

        for instance in plugins {
            match instance
                .plugin
                .execute(phase, ctx, &instance.config)
                .await
            {
                PluginResult::Continue => {}
                PluginResult::Response {
                    status,
                    headers,
                    body,
                } => {
                    return PluginResult::Response {
                        status,
                        headers,
                        body,
                    };
                }
                PluginResult::Error(msg) => {
                    error!(
                        plugin = %instance.name,
                        phase = %phase,
                        error = %msg,
                        "Plugin execution error"
                    );
                    return PluginResult::Error(msg);
                }
            }
        }

        PluginResult::Continue
    }

    /// Execute all pre-proxy phases in order: Rewrite -> Access -> BeforeProxy.
    pub async fn execute_request_phases(&self, ctx: &mut PluginContext) -> PluginResult {
        for phase in &[Phase::Rewrite, Phase::Access, Phase::BeforeProxy] {
            match self.execute_phase(*phase, ctx).await {
                PluginResult::Continue => {}
                other => return other,
            }
        }
        PluginResult::Continue
    }

    /// Execute response phases: HeaderFilter -> BodyFilter.
    pub async fn execute_response_phases(&self, ctx: &mut PluginContext) -> PluginResult {
        for phase in &[Phase::HeaderFilter, Phase::BodyFilter] {
            match self.execute_phase(*phase, ctx).await {
                PluginResult::Continue => {}
                other => return other,
            }
        }
        PluginResult::Continue
    }

    /// Execute the log phase (always runs, errors are logged but not returned).
    pub async fn execute_log_phase(&self, ctx: &mut PluginContext) {
        if let Some(plugins) = self.phases.get(&Phase::Log) {
            for instance in plugins {
                match instance
                    .plugin
                    .execute(Phase::Log, ctx, &instance.config)
                    .await
                {
                    PluginResult::Error(msg) => {
                        warn!(
                            plugin = %instance.name,
                            error = %msg,
                            "Log phase plugin error (non-fatal)"
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    /// Get the number of plugin instances across all phases.
    pub fn plugin_count(&self) -> usize {
        self.phases.values().map(|v| v.len()).sum()
    }
}
