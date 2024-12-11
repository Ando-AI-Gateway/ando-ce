use ando_core::config::GatewayConfig;
use ando_core::router::Router;
use ando_plugin::registry::PluginRegistry;
use ando_store::cache::ConfigCache;
use arc_swap::ArcSwap;
use std::sync::Arc;
use tracing::{error, info};

/// Shared state that all monoio worker threads can read.
///
/// v2 design: The ArcSwap<Router> is the ONLY shared mutable state.
/// It's updated by the control plane thread and read by worker threads
/// via a single atomic load (no lock, no CAS, no contention).
pub struct SharedState {
    pub router: Arc<ArcSwap<Router>>,
    pub plugin_registry: Arc<PluginRegistry>,
    pub config_cache: ConfigCache,
    pub config: Arc<GatewayConfig>,
}

impl SharedState {
    pub fn new(
        router: Router,
        plugin_registry: PluginRegistry,
        config_cache: ConfigCache,
        config: GatewayConfig,
    ) -> Arc<Self> {
        Arc::new(Self {
            router: Arc::new(ArcSwap::new(Arc::new(router))),
            plugin_registry: Arc::new(plugin_registry),
            config_cache,
            config: Arc::new(config),
        })
    }
}

/// Spawn monoio worker threads — one per core.
///
/// Each thread runs an independent monoio runtime with its own
/// TCP listener (via SO_REUSEPORT), event loop, and proxy state.
pub fn spawn_workers(
    shared: Arc<SharedState>,
    num_workers: usize,
) -> Vec<std::thread::JoinHandle<()>> {
    let listen_addr = shared.config.proxy.http_addr.clone();
    let mut handles = Vec::with_capacity(num_workers);

    for worker_id in 0..num_workers {
        let shared = Arc::clone(&shared);
        let addr = listen_addr.clone();

        let handle = std::thread::Builder::new()
            .name(format!("ando-worker-{}", worker_id))
            .spawn(move || {
                // Each thread gets its own monoio runtime
                let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
                    .enable_all()
                    .build()
                    .expect("Failed to build monoio runtime");

                rt.block_on(worker_loop(worker_id, shared, addr));
            })
            .expect("Failed to spawn worker thread");

        handles.push(handle);
    }

    info!(workers = num_workers, addr = %listen_addr, "Workers spawned");
    handles
}

/// Main loop for a single worker thread.
async fn worker_loop(worker_id: usize, shared: Arc<SharedState>, addr: String) {
    use monoio::net::TcpListener;

    let listener = TcpListener::bind(&addr).expect(&format!(
        "Worker {} failed to bind to {}",
        worker_id, addr
    ));

    info!(worker = worker_id, addr = %addr, "Worker listening");

    // Create thread-local proxy worker
    let mut proxy = crate::proxy::ProxyWorker::new(
        shared.router.load_full(),
        Arc::clone(&shared.plugin_registry),
        shared.config_cache.clone(),
        Arc::clone(&shared.config),
    );

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                // Check for router updates (cheap atomic load)
                let current_router = shared.router.load_full();
                proxy.maybe_update_router(current_router);

                // Handle connection
                let shared_ref = Arc::clone(&shared);
                monoio::spawn(async move {
                    if let Err(e) =
                        crate::connection::handle_connection(stream, peer_addr, &shared_ref).await
                    {
                        // Connection errors are normal (client disconnect, etc.)
                        tracing::debug!(error = %e, "Connection error");
                    }
                });
            }
            Err(e) => {
                error!(worker = worker_id, error = %e, "Accept error");
            }
        }
    }
}
