use crate::handlers;
use ando_core::config::AdminConfig;
use ando_core::router::Router;
use ando_observability::MetricsCollector;
use ando_plugin::registry::PluginRegistry;
use ando_store::cache::ConfigCache;
use axum::{
    extract::State,
    routing::{delete, get, post, put},
    Router as AxumRouter,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tracing::info;

/// Shared state for the Admin API.
#[derive(Clone)]
pub struct AppState {
    pub cache: ConfigCache,
    pub router: Arc<Router>,
    pub metrics: Arc<MetricsCollector>,
    pub plugin_registry: Arc<PluginRegistry>,
    pub store: Arc<Mutex<Option<ando_store::EtcdStore>>>,
}

/// Admin REST API server.
pub struct AdminServer {
    config: AdminConfig,
    state: AppState,
}

impl AdminServer {
    pub fn new(
        config: AdminConfig,
        cache: ConfigCache,
        router: Arc<Router>,
        metrics: Arc<MetricsCollector>,
        plugin_registry: Arc<PluginRegistry>,
        store: Option<ando_store::EtcdStore>,
    ) -> Self {
        let state = AppState {
            cache,
            router,
            metrics,
            plugin_registry,
            store: Arc::new(Mutex::new(store)),
        };

        Self { config, state }
    }

    /// Build the Axum router with all admin routes.
    fn build_router(&self) -> AxumRouter {
        let admin_api = AxumRouter::new()
            // Health
            .route("/health", get(handlers::health::health_check))
            .route("/schema", get(handlers::health::schema))
            // Routes
            .route("/routes", get(handlers::routes::list_routes))
            .route("/routes", post(handlers::routes::create_route))
            .route("/routes/{id}", get(handlers::routes::get_route))
            .route("/routes/{id}", put(handlers::routes::update_route))
            .route("/routes/{id}", delete(handlers::routes::delete_route))
            // Services
            .route("/services", get(handlers::services::list_services))
            .route("/services", post(handlers::services::create_service))
            .route("/services/{id}", get(handlers::services::get_service))
            .route("/services/{id}", put(handlers::services::update_service))
            .route("/services/{id}", delete(handlers::services::delete_service))
            // Upstreams
            .route("/upstreams", get(handlers::upstreams::list_upstreams))
            .route("/upstreams", post(handlers::upstreams::create_upstream))
            .route("/upstreams/{id}", get(handlers::upstreams::get_upstream))
            .route("/upstreams/{id}", put(handlers::upstreams::update_upstream))
            .route("/upstreams/{id}", delete(handlers::upstreams::delete_upstream))
            // Consumers
            .route("/consumers", get(handlers::consumers::list_consumers))
            .route("/consumers", post(handlers::consumers::create_consumer))
            .route("/consumers/{id}", get(handlers::consumers::get_consumer))
            .route("/consumers/{id}", delete(handlers::consumers::delete_consumer))
            // SSL
            .route("/ssls", get(handlers::ssl::list_ssl))
            .route("/ssls", post(handlers::ssl::create_ssl))
            .route("/ssls/{id}", get(handlers::ssl::get_ssl))
            .route("/ssls/{id}", delete(handlers::ssl::delete_ssl))
            // Plugins
            .route("/plugins/list", get(handlers::plugins::list_plugins));

        AxumRouter::new()
            .nest("/apisix/admin", admin_api)
            .route("/metrics", get(metrics_handler))
            .fallback_service(ServeDir::new("ando-ui/out"))
            .with_state(self.state.clone())
    }

    /// Start the admin API server.
    pub async fn start(self) -> anyhow::Result<()> {
        if !self.config.enabled {
            info!("Admin API disabled");
            return Ok(());
        }

        let addr = self.config.addr;
        let app = self.build_router();

        info!(addr = %addr, "Starting Admin API server");

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

/// Prometheus metrics endpoint handler.
async fn metrics_handler(State(state): State<AppState>) -> String {
    state.metrics.gather_text()
}
