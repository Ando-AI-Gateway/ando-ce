use mlua::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Pool of pre-warmed LuaJIT VMs for high-performance plugin execution.
///
/// Each VM is fully initialized with the PDK and can execute plugin code
/// without the overhead of VM creation. The pool prevents contention
/// by maintaining multiple VMs that can execute concurrently.
pub struct LuaVmPool {
    pool: Arc<Mutex<Vec<Lua>>>,
    pool_size: usize,
    max_memory: usize,
}

impl LuaVmPool {
    /// Create a new pool with `size` pre-warmed VMs.
    pub fn new(size: usize, max_memory: usize) -> anyhow::Result<Self> {
        info!(
            pool_size = size,
            max_memory = max_memory,
            "Initializing Lua VM pool"
        );

        let mut vms = Vec::with_capacity(size);
        for i in 0..size {
            let lua = Self::create_vm(max_memory)?;
            debug!(vm_index = i, "Created Lua VM");
            vms.push(lua);
        }

        Ok(Self {
            pool: Arc::new(Mutex::new(vms)),
            pool_size: size,
            max_memory,
        })
    }

    /// Acquire a VM from the pool. Blocks if all VMs are in use.
    pub async fn acquire(&self) -> anyhow::Result<Lua> {
        let mut pool = self.pool.lock().await;
        if let Some(vm) = pool.pop() {
            Ok(vm)
        } else {
            // Pool exhausted — create a temporary VM
            warn!("Lua VM pool exhausted, creating temporary VM");
            Self::create_vm(self.max_memory)
        }
    }

    /// Return a VM to the pool.
    pub async fn release(&self, vm: Lua) {
        let mut pool = self.pool.lock().await;
        if pool.len() < self.pool_size {
            // Reset the VM state before returning (lightweight cleanup)
            if let Err(e) = Self::reset_vm(&vm) {
                warn!(error = %e, "Failed to reset Lua VM, discarding");
                return;
            }
            pool.push(vm);
        }
        // If pool is full, the VM is dropped (temp VMs created during exhaustion)
    }

    /// Create a new LuaJIT VM with PDK pre-loaded.
    fn create_vm(max_memory: usize) -> anyhow::Result<Lua> {
        let lua = if max_memory > 0 {
            Lua::new_with(
                mlua::StdLib::ALL_SAFE,
                LuaOptions::new(),
            )?
        } else {
            Lua::new()
        };

        // Set memory limit if configured
        if max_memory > 0 {
            lua.set_memory_limit(max_memory)?;
        }

        // Pre-load the PDK module into the VM
        super::pdk::register_pdk(&lua)?;

        Ok(lua)
    }

    /// Lightweight VM reset between uses.
    fn reset_vm(lua: &Lua) -> anyhow::Result<()> {
        // Clear any per-request state
        lua.load(
            r#"
            if _G.__ando_ctx then
                _G.__ando_ctx = nil
            end
            if _G.__ando_response then
                _G.__ando_response = nil
            end
            collectgarbage("collect")
            "#,
        )
        .exec()?;
        Ok(())
    }

    /// Get pool statistics.
    pub async fn stats(&self) -> LuaPoolStats {
        let pool = self.pool.lock().await;
        LuaPoolStats {
            pool_size: self.pool_size,
            available: pool.len(),
            in_use: self.pool_size - pool.len(),
        }
    }
}

#[derive(Debug)]
pub struct LuaPoolStats {
    pub pool_size: usize,
    pub available: usize,
    pub in_use: usize,
}
