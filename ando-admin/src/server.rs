use crate::handlers;
use ando_core::config::AdminConfig;
use ando_core::router::Router;
use ando_plugin::registry::PluginRegistry;
use ando_store::cache::ConfigCache;
use arc_swap::ArcSwap;
use axum::{
    routing::{delete, get, put},
    Router as AxumRouter,
};
use http::Method;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Notify;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

/// Shared state for the admin API.
pub struct AdminState {
    pub cache: ConfigCache,
    pub router_swap: Arc<ArcSwap<Router>>,
    pub plugin_registry: Arc<PluginRegistry>,
    /// Signal worker threads that config has changed.
    pub config_changed: Arc<Notify>,
    /// Path to the JSON file used for persistence (standalone mode).
    /// `None` in unit-test contexts — persistence is skipped.
    pub state_file: Option<PathBuf>,
}

/// Start the admin API server on a dedicated tokio runtime.
///
/// v2 design: The admin API is completely separate from the data plane.
/// It runs on tokio (for axum compatibility) in its own thread. Config
/// changes are applied to the shared ConfigCache + ArcSwap<Router>,
/// which worker cores pick up via atomic loads.
pub async fn start_admin(
    config: AdminConfig,
    state: Arc<AdminState>,
) -> anyhow::Result<()> {
    let app = build_admin_router(state);

    let listener = tokio::net::TcpListener::bind(&config.addr).await?;
    info!(addr = %config.addr, "Admin API listening");

    axum::serve(listener, app).await?;
    Ok(())
}

/// Build the admin Axum router with all APISIX-compatible routes.
/// Extracted so tests can call this without binding a real port.
pub fn build_admin_router(state: Arc<AdminState>) -> AxumRouter {
    AxumRouter::new()
        // Dashboard UI (Next.js static export, embedded at compile time)
        .route("/dashboard", get(handlers::dashboard::dashboard_index))
        .route("/dashboard/{*path}", get(handlers::dashboard::dashboard_assets))
        // APISIX-compatible admin API routes
        .route("/apisix/admin/routes/{id}", put(handlers::routes::put_route))
        .route("/apisix/admin/routes/{id}", get(handlers::routes::get_route))
        .route("/apisix/admin/routes/{id}", delete(handlers::routes::delete_route))
        .route("/apisix/admin/routes", get(handlers::routes::list_routes))
        .route("/apisix/admin/upstreams/{id}", put(handlers::upstreams::put_upstream))
        .route("/apisix/admin/upstreams/{id}", get(handlers::upstreams::get_upstream))
        .route("/apisix/admin/upstreams/{id}", delete(handlers::upstreams::delete_upstream))
        .route("/apisix/admin/upstreams", get(handlers::upstreams::list_upstreams))
        .route("/apisix/admin/consumers/{username}", put(handlers::consumers::put_consumer))
        .route("/apisix/admin/consumers/{username}", get(handlers::consumers::get_consumer))
        .route("/apisix/admin/consumers/{username}", delete(handlers::consumers::delete_consumer))
        .route("/apisix/admin/consumers", get(handlers::consumers::list_consumers))
        .route("/apisix/admin/health", get(handlers::health::health_check))
        .route("/apisix/admin/plugins/list", get(handlers::plugins::list_plugins))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::PUT, Method::DELETE, Method::OPTIONS])
                .allow_headers(Any),
        )
        .with_state(state)
}
