use ando_admin::AdminServer;
use ando_core::config::{AndoConfig, DeploymentMode};
use ando_core::router::Router;
use ando_observability::{MetricsCollector, VictoriaLogsExporter};
use ando_plugin::lua::pool::LuaVmPool;
use ando_plugin::lua::runtime::LuaPluginRuntime;
use ando_plugin::registry::PluginRegistry;
use ando_store::cache::ConfigCache;
use ando_store::watcher::ConfigWatcher;
use ando_store::EtcdStore;
use clap::Parser;
use std::sync::Arc;
use tracing::{error, info};

// Use jemalloc as the global allocator — reduces contention under
// multi-threaded workloads compared to the system allocator.
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser)]
#[command(name = "ando")]
#[command(about = "Ando — Enterprise API Gateway built on Pingora")]
#[command(version)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "config/ando.yaml")]
    config: String,

    /// Deployment mode override
    #[arg(short, long)]
    mode: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .json()
        .init();

    info!(
        version = env!("CARGO_PKG_VERSION"),
        config = %cli.config,
        "Starting Ando API Gateway"
    );

    // Load configuration
    let mut config = AndoConfig::load(Some(&cli.config))
        .unwrap_or_else(|e| {
            info!(error = %e, "Could not load config file, using defaults");
            AndoConfig::default()
        });

    // Override deployment mode if specified
    if let Some(ref mode) = cli.mode {
        config.deployment.mode = match mode.as_str() {
            "standalone" => DeploymentMode::Standalone,
            "edge" => DeploymentMode::Edge,
            _ => DeploymentMode::Standard,
        };
    }

    info!(
        mode = ?config.deployment.mode,
        http_addr = %config.proxy.http_addr,
        admin_addr = %config.admin.addr,
        node_id = %config.node_id,
        "Configuration loaded"
    );

    // Create a Pingora server with an empty configuration override to avoid argument conflicts
    let opt = pingora_core::server::configuration::Opt::default();
    let mut server = pingora::server::Server::new(Some(opt))?;

    // Apply worker thread count from config BEFORE bootstrap.
    // Defaults to 0 in config which means "auto" (= number of logical CPUs).
    // Without this the Pingora default is 1 thread, which is why throughput
    // plateaus at ~9K req/s regardless of concurrency.
    let worker_count = if config.proxy.workers == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    } else {
        config.proxy.workers
    };
    if let Some(conf) = Arc::get_mut(&mut server.configuration) {
        conf.threads = worker_count;

        // ── Upstream connection pool ─────────────────────────────────────────
        // The MOST important setting for throughput.  Default (0) means no
        // pooling → new TCP connection per request.  Each worker gets its own
        // pool, so effective total = pool_size × workers.
        conf.upstream_keepalive_pool_size = 1024;

        // ── Disable work-stealing ────────────────────────────────────────────
        // Each worker thread handles only its own tasks.  This eliminates
        // cross-thread synchronisation and improves CPU cache locality for
        // proxy workloads where every request is independent.
        conf.work_stealing = false;

        // ── No connect offload ───────────────────────────────────────────────
        // With a large keepalive pool, virtually all upstream connections are
        // reused.  Offloading the rare new connection to a dedicated thread
        // pool just wastes 4 threads of CPU and adds context-switch latency.
        // Disabled for maximum throughput.
        conf.grace_period_seconds = Some(5);
        conf.graceful_shutdown_timeout_seconds = Some(5);
    }
    info!(workers = worker_count, "Worker threads configured");

    server.bootstrap();

    // Initialize shared components
    let router = Arc::new(Router::new());
    let cache = ConfigCache::new();
    let plugin_registry = Arc::new(PluginRegistry::new());
    let metrics = Arc::new(MetricsCollector::new()?);
    let logs_exporter = Arc::new(VictoriaLogsExporter::new(
        config.observability.victoria_logs.clone(),
    ));

    // Register built-in plugins
    ando_plugins::register_all(&plugin_registry);

    // Initialize Lua VM pool and load Lua plugins
    let lua_pool = Arc::new(LuaVmPool::new(
        config.lua.pool_size,
        config.lua.max_memory,
    )?);

    let lua_runtime = LuaPluginRuntime::new(
        Arc::clone(&lua_pool),
        config.lua.plugin_dir.clone(),
    );

    info!(
        lua_pool_size = config.lua.pool_size,
        lua_plugin_dir = %config.lua.plugin_dir.display(),
        "Lua PDK initialized"
    );

    // Build the proxy service
    let proxy = ando_proxy::AndoProxy::new(
        Arc::clone(&router),
        cache.clone(),
        Arc::clone(&plugin_registry),
        Arc::clone(&metrics),
        Arc::clone(&logs_exporter),
    );

    // Create Pingora HTTP proxy service with optimised settings
    let mut proxy_service = pingora_proxy::http_proxy_service(
        &server.configuration,
        proxy,
    );

    proxy_service.add_tcp(&config.proxy.http_addr.to_string());
    info!(addr = %config.proxy.http_addr, "HTTP proxy listener configured");

    server.add_service(proxy_service);

    // Start VictoriaMetrics push (if enabled)
    if config.observability.victoria_metrics.enabled {
        Arc::clone(&metrics).start_push_loop(config.observability.victoria_metrics.clone());
    }

    // Spawn background tasks via Pingora's runtime
    let etcd_config = config.etcd.clone();
    let admin_config = config.admin.clone();
    let cache_clone = cache.clone();
    let router_clone = Arc::clone(&router);
    let metrics_clone = Arc::clone(&metrics);
    let registry_clone = Arc::clone(&plugin_registry);
    let is_standalone = config.is_standalone();

    // Background services (etcd watcher, admin API, Lua plugin discovery)
    let background = pingora_core::services::background::background_service(
        "ando-background",
        AndoBackground {
            etcd_config,
            admin_config,
            cache: cache_clone,
            router: router_clone,
            metrics: metrics_clone,
            plugin_registry: registry_clone,
            is_standalone,
            lua_runtime,
        },
    );

    server.add_service(background);

    info!("All services configured. Starting Pingora server...");
    server.run_forever();
}

/// Background service that runs etcd sync, admin API, and plugin discovery.
struct AndoBackground {
    etcd_config: ando_core::config::EtcdConfig,
    admin_config: ando_core::config::AdminConfig,
    cache: ConfigCache,
    router: Arc<Router>,
    metrics: Arc<MetricsCollector>,
    plugin_registry: Arc<PluginRegistry>,
    is_standalone: bool,
    lua_runtime: LuaPluginRuntime,
}

#[async_trait::async_trait]
impl pingora_core::services::background::BackgroundService for AndoBackground {
    async fn start(&self, mut _shutdown: pingora_core::server::ShutdownWatch) {
        // Discover and register Lua plugins
        match self.lua_runtime.discover_plugins().await {
            Ok(plugins) => {
                for plugin in plugins {
                    self.plugin_registry.register(Arc::new(plugin));
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to discover Lua plugins");
            }
        }

        if self.is_standalone {
            info!("Running in standalone mode — etcd sync disabled");

            // Start Admin API without etcd
            let admin = AdminServer::new(
                self.admin_config.clone(),
                self.cache.clone(),
                Arc::clone(&self.router),
                Arc::clone(&self.metrics),
                Arc::clone(&self.plugin_registry),
                None,
            );

            if let Err(e) = admin.start().await {
                error!(error = %e, "Admin API server error");
            }
            return;
        }

        // Standard mode — etcd sync + Admin API with retries
        let watcher = ConfigWatcher::new(self.etcd_config.clone(), self.cache.clone());
        let admin_config = self.admin_config.clone();
        let cache = self.cache.clone();
        let router = Arc::clone(&self.router);
        let metrics = Arc::clone(&self.metrics);
        let registry = Arc::clone(&self.plugin_registry);
        let etcd_config = self.etcd_config.clone();

        // Spawn Admin API in its own task
        tokio::spawn(async move {
            loop {
                let etcd_store = EtcdStore::connect(&etcd_config).await.ok();
                let admin = AdminServer::new(
                    admin_config.clone(),
                    cache.clone(),
                    Arc::clone(&router),
                    Arc::clone(&metrics),
                    Arc::clone(&registry),
                    etcd_store,
                );

                info!("Starting Admin API server");
                if let Err(e) = admin.start().await {
                    error!(error = %e, "Admin API server error, retrying in 5s");
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                } else {
                    break; // Server exited normally (if that's possible)
                }
            }
        });

        // Loop for initial sync and watch
        loop {
            // Initial sync
            if let Err(e) = watcher.initial_sync().await {
                error!(error = %e, "Failed initial etcd sync, retrying in 5s");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }

            // Sync successful, rebuild router
            let routes: Vec<_> = self.cache.routes.iter().map(|r| r.value().clone()).collect();
            if let Err(e) = self.router.replace_all(routes) {
                error!(error = %e, "Failed to rebuild router after initial sync");
            }

            // Watch forever (or until error)
            info!("Entering etcd watch loop");
            if let Err(e) = watcher.watch_forever().await {
                error!(error = %e, "etcd watch error, restarting sync cycle in 5s");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }
}
