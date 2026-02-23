// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Ando CE — Zero-Overhead API Gateway
//
//  Architecture: monoio thread-per-core + shared-nothing data plane
//  Admin API:    axum on dedicated tokio thread
//  Config:       standalone YAML / etcd with watch
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use ando_core::config::GatewayConfig;
use ando_core::router::Router;
use ando_plugin::registry::PluginRegistry;
use ando_proxy::worker::{self, SharedState};
use ando_store::cache::ConfigCache;
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::info;

/// Global shutdown flag — checked by signal handler.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

#[derive(Parser, Debug)]
#[command(name = "ando", version, about = "Ando CE — Zero-Overhead API Gateway")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "/etc/ando/ando.yaml")]
    config: PathBuf,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Path to the JSON state file used for persistence (routes, upstreams, consumers).
    /// Data written via the Admin API is saved here and reloaded on restart.
    #[arg(long, default_value = "data/ando-state.json")]
    state_file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // ── Tracing ──
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log_level)),
        )
        .with_target(false)
        .init();

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Ando CE starting — monoio thread-per-core engine"
    );

    // ── Config ──
    let config = if cli.config.exists() {
        info!(path = %cli.config.display(), "Loading config file");
        GatewayConfig::load(&cli.config)?
    } else {
        info!("No config file found, using defaults");
        GatewayConfig::default()
    };

    let num_workers = config.effective_workers();
    info!(workers = num_workers, "Worker count");

    // ── Plugin registry ──
    let mut registry = PluginRegistry::new();
    ando_plugins::register_all(&mut registry);
    info!(plugins = registry.len(), "Plugins registered");

    // ── Config cache ──
    let cache = ConfigCache::new();

    // ── Restore persisted state (routes / upstreams / consumers) ──
    ando_admin::persist::load_state(&cli.state_file, &cache);

    // ── Initial router (built from persisted routes, or empty) ──
    let initial_routes = cache.all_routes();
    let router = Router::build(initial_routes, 0)?;

    // ── Shared state ──
    let shared = SharedState::new(router, registry, cache.clone(), config.clone());

    // ── Admin API state ──
    let config_changed = Arc::new(Notify::new());
    let admin_state = Arc::new(ando_admin::server::AdminState {
        cache: cache.clone(),
        router_swap: Arc::clone(&shared.router),
        plugin_registry: Arc::clone(&shared.plugin_registry),
        config_changed: config_changed.clone(),
        state_file: Some(cli.state_file.clone()),
    });

    // ── Start admin API on a dedicated tokio thread ──
    let admin_config = config.admin.clone();
    if admin_config.enabled {
        let admin_state = Arc::clone(&admin_state);
        std::thread::Builder::new()
            .name("ando-admin".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime for admin");

                rt.block_on(async {
                    if let Err(e) = ando_admin::server::start_admin(admin_config, admin_state).await
                    {
                        tracing::error!(error = %e, "Admin API failed");
                    }
                });
            })
            .expect("Failed to spawn admin thread");

        info!(addr = %config.admin.addr, "Admin API started");
    }

    // ── Spawn monoio worker threads ──
    let worker_handles = worker::spawn_workers(Arc::clone(&shared), num_workers);

    info!(
        workers = num_workers,
        proxy_addr = %config.proxy.http_addr,
        admin_addr = %config.admin.addr,
        "Ando CE is ready — serving traffic"
    );

    // ── Graceful shutdown: wait for SIGTERM/SIGINT ──
    setup_signal_handler();

    // Wait for shutdown signal
    while !SHUTDOWN.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    info!("Shutdown signal received, stopping...");

    // In the current architecture, workers run in an infinite accept loop.
    // On process exit, all threads are cleaned up by the OS.
    // Future improvement: send shutdown notification to each worker.
    drop(worker_handles);

    info!("Ando CE stopped");
    Ok(())
}

fn setup_signal_handler() {
    // SIGTERM (docker stop) + SIGINT (Ctrl+C)
    for sig in [libc::SIGTERM, libc::SIGINT] {
        unsafe {
            libc::signal(sig, signal_handler as libc::sighandler_t);
        }
    }
}

extern "C" fn signal_handler(_sig: libc::c_int) {
    SHUTDOWN.store(true, Ordering::Relaxed);
}
