use crate::lua::pdk;
use crate::lua::pool::LuaVmPool;
use crate::plugin::{Phase, Plugin, PluginContext, PluginResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Lua plugin runtime — manages loading and executing Lua plugins.
///
/// Each Lua plugin is a `.lua` file that defines handlers for the various
/// execution phases. The runtime uses the VM pool for efficient execution.
pub struct LuaPluginRuntime {
    pool: Arc<LuaVmPool>,
    /// Cached source for loaded plugins: plugin_name -> source
    source_cache: dashmap::DashMap<String, String>,
    plugin_dir: PathBuf,
}

impl LuaPluginRuntime {
    pub fn new(pool: Arc<LuaVmPool>, plugin_dir: PathBuf) -> Self {
        Self {
            pool,
            source_cache: dashmap::DashMap::new(),
            plugin_dir,
        }
    }

    /// Load a Lua plugin from file and cache its source.
    pub async fn load_plugin(&self, name: &str) -> anyhow::Result<LuaPlugin> {
        let plugin_path = self.plugin_dir.join(format!("{}.lua", name));

        if !plugin_path.exists() {
            anyhow::bail!("Lua plugin file not found: {}", plugin_path.display());
        }

        let source = tokio::fs::read_to_string(&plugin_path).await?;

        // Cache the source
        self.source_cache
            .insert(name.to_string(), source.clone());

        info!(plugin = name, path = %plugin_path.display(), "Loaded Lua plugin");

        Ok(LuaPlugin {
            name: name.to_string(),
            source,
            pool: Arc::clone(&self.pool),
        })
    }

    /// Discover and load all Lua plugins from the plugin directory.
    pub async fn discover_plugins(&self) -> anyhow::Result<Vec<LuaPlugin>> {
        let mut plugins = Vec::new();

        if !self.plugin_dir.exists() {
            warn!(dir = %self.plugin_dir.display(), "Lua plugin directory does not exist");
            return Ok(plugins);
        }

        let mut entries = tokio::fs::read_dir(&self.plugin_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("lua") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    match self.load_plugin(stem).await {
                        Ok(plugin) => plugins.push(plugin),
                        Err(e) => {
                            error!(plugin = stem, error = %e, "Failed to load Lua plugin");
                        }
                    }
                }
            }
        }

        info!(count = plugins.len(), "Discovered Lua plugins");
        Ok(plugins)
    }
}

/// A compiled Lua plugin ready for execution.
pub struct LuaPlugin {
    name: String,
    source: String,
    pool: Arc<LuaVmPool>,
}

/// Execute the Lua plugin synchronously within a VM.
/// This is called inside `tokio::task::spawn_blocking` to avoid Send issues.
fn execute_lua_plugin(
    vm: &mlua::Lua,
    source: &str,
    phase: Phase,
    method: &str,
    uri: &str,
    path: &str,
    query: &str,
    headers: &HashMap<String, String>,
    client_ip: &str,
    body: Option<&[u8]>,
    vars: &HashMap<String, serde_json::Value>,
    config: &serde_json::Value,
) -> Result<LuaExecResult, String> {
    // Set up request context
    if let Err(e) = pdk::setup_request_context(
        vm, method, uri, path, query, headers, client_ip, body, vars,
    ) {
        return Err(format!("Failed to set up Lua context: {}", e));
    }

    // Set plugin config
    if let Err(e) = (|| -> Result<(), mlua::Error> {
        let config_lua = pdk::json_to_lua_value(vm, config)?;
        vm.globals().set("__ando_plugin_config", config_lua)?;
        Ok(())
    })() {
        return Err(format!("Failed to set plugin config: {}", e));
    }

    // Load and execute the plugin
    let result = (|| -> Result<LuaExecResult, mlua::Error> {
        let chunk = vm.load(source);
        let plugin_table: mlua::Value = chunk.call(())?;

        // Call the phase handler if it exists
        if let mlua::Value::Table(ref t) = plugin_table {
            let phase_name = phase.as_str();
            if let Ok(handler) = t.get::<mlua::Function>(phase_name) {
                let config_val: mlua::Value =
                    vm.globals().get("__ando_plugin_config")?;
                handler.call::<()>((config_val, mlua::Value::Nil))?;
            }
        }

        // Check if the plugin set a response (short-circuit)
        if let Ok(Some(resp_state)) = pdk::read_response_state(vm) {
            Ok(LuaExecResult::Response {
                status: resp_state.status,
                headers: resp_state.headers,
                body: resp_state.body,
            })
        } else {
            // Read back modified headers
            let mut modified_headers = HashMap::new();
            if let Ok(ctx_table) = vm.globals().get::<mlua::Table>("__ando_ctx") {
                if let Ok(h) = ctx_table.get::<mlua::Table>("headers") {
                    if let Ok(pairs) =
                        h.pairs::<String, String>().collect::<Result<Vec<_>, _>>()
                    {
                        for (k, v) in pairs {
                            modified_headers.insert(k, v);
                        }
                    }
                }
            }

            Ok(LuaExecResult::Continue {
                modified_headers: if modified_headers.is_empty() {
                    None
                } else {
                    Some(modified_headers)
                },
            })
        }
    })();

    match result {
        Ok(r) => Ok(r),
        Err(e) => Err(format!("Lua error: {}", e)),
    }
}

/// Result from Lua execution (Send-safe).
#[derive(Debug)]
enum LuaExecResult {
    Continue {
        modified_headers: Option<HashMap<String, String>>,
    },
    Response {
        status: u16,
        headers: HashMap<String, String>,
        body: Option<String>,
    },
}

#[async_trait]
impl Plugin for LuaPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        100
    }

    fn phases(&self) -> Vec<Phase> {
        vec![
            Phase::Rewrite,
            Phase::Access,
            Phase::BeforeProxy,
            Phase::HeaderFilter,
            Phase::BodyFilter,
            Phase::Log,
        ]
    }

    async fn execute(
        &self,
        phase: Phase,
        ctx: &mut PluginContext,
        config: &serde_json::Value,
    ) -> PluginResult {
        // Capture all data we need to pass into the blocking task
        let source = self.source.clone();
        let method = ctx.request_method.clone();
        let uri = ctx.request_uri.clone();
        let path = ctx.request_path.clone();
        let query = ctx.request_query.clone();
        let headers = ctx.request_headers.clone();
        let client_ip = ctx.client_ip.clone();
        let body = ctx.request_body.clone();
        let vars = ctx.vars.clone();
        let config = config.clone();
        let _pool = Arc::clone(&self.pool);
        let plugin_name = self.name.clone();

        // Execute in a blocking task to avoid Send constraints on Lua VM
        let result = tokio::task::spawn_blocking(move || {
            // Create a temporary VM for this execution
            // (pooling with spawn_blocking is complex; using fresh VMs for correctness)
            let vm = mlua::Lua::new();

            // Register PDK
            if let Err(e) = pdk::register_pdk(&vm) {
                return Err(format!("Failed to register PDK: {}", e));
            }

            execute_lua_plugin(
                &vm,
                &source,
                phase,
                &method,
                &uri,
                &path,
                &query,
                &headers,
                &client_ip,
                body.as_deref(),
                &vars,
                &config,
            )
        })
        .await;

        match result {
            Ok(Ok(LuaExecResult::Continue { modified_headers })) => {
                // Apply modified headers back to context
                if let Some(headers) = modified_headers {
                    ctx.request_headers = headers;
                }
                PluginResult::Continue
            }
            Ok(Ok(LuaExecResult::Response {
                status,
                headers,
                body,
            })) => PluginResult::Response {
                status,
                headers,
                body: body.map(|s| s.into_bytes()),
            },
            Ok(Err(msg)) => {
                PluginResult::Error(format!("Lua plugin '{}' error: {}", plugin_name, msg))
            }
            Err(e) => {
                PluginResult::Error(format!("Lua plugin '{}' task error: {}", plugin_name, e))
            }
        }
    }
}
