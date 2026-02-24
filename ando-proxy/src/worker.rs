use ando_core::config::GatewayConfig;
use ando_core::router::Router;
use ando_plugin::registry::PluginRegistry;
use ando_store::cache::ConfigCache;
use arc_swap::ArcSwap;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{error, info};

use crate::proxy::{ConnPool, ProxyWorker};

/// Shared state across all worker threads.
///
/// The ArcSwap<Router> is the ONLY shared mutable state.
/// Updated by admin API, read by workers via atomic load.
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
///
/// Creates ONE ProxyWorker and ONE ConnPool for this thread.
/// All connections on this thread share them via Rc<RefCell>.
///
/// Pool is pre-warmed before accepting any traffic.
async fn worker_loop(worker_id: usize, shared: Arc<SharedState>, addr: String) {
    use monoio::net::TcpListener;

    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        panic!("Worker {} failed to bind to {}: {}", worker_id, addr, e);
    });

    info!(worker = worker_id, addr = %addr, "Worker listening");

    // ── Create ONCE per thread ──
    let pool_size = shared.config.proxy.keepalive_pool_size;
    let proxy_inner = ProxyWorker::new(
        shared.router.load_full(),
        Arc::clone(&shared.plugin_registry),
        shared.config_cache.clone(),
        Arc::clone(&shared.config),
    );

    // ── Pre-warm connection pool ──
    let upstream_addrs = proxy_inner.upstream_addresses();
    let mut pool_inner = ConnPool::new(pool_size);
    let warm_count = (pool_size / 2).max(8).min(pool_size); // warm half the pool
    pool_inner.warm(&upstream_addrs, warm_count).await;

    let proxy = Rc::new(RefCell::new(proxy_inner));
    let conn_pool = Rc::new(RefCell::new(pool_inner));

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                // TCP_NODELAY — disable Nagle's for lowest latency
                let _ = stream.set_nodelay(true);

                // Check for router updates (cheap atomic load)
                {
                    let current = shared.router.load_full();
                    proxy.borrow_mut().maybe_update_router(current);
                }

                let proxy = Rc::clone(&proxy);
                let pool = Rc::clone(&conn_pool);

                monoio::spawn(async move {
                    if let Err(e) =
                        crate::connection::handle_connection(stream, peer_addr, proxy, pool).await
                    {
                        tracing::debug!(error = %e, "Connection closed");
                    }
                });
            }
            Err(e) => {
                error!(worker = worker_id, error = %e, "Accept error");
            }
        }
    }
}
