//! Integration tests for the Admin REST API handlers.
//!
//! Uses `tower::ServiceExt::oneshot` to call handlers without binding a real
//! TCP port — every test gets a fresh in-memory state.

use ando_admin::server::{build_admin_router, AdminState};
use ando_core::router::Router;
use ando_plugin::registry::PluginRegistry;
use ando_store::cache::ConfigCache;
use arc_swap::ArcSwap;
use axum::body::{to_bytes, Body};
use axum::http::{Method, Request, StatusCode};
use std::sync::Arc;
use tokio::sync::Notify;
use tower::ServiceExt; // .oneshot()

// ── Helper ────────────────────────────────────────────────────

fn make_state() -> Arc<AdminState> {
    let cache = ConfigCache::new();
    let initial_router = Router::build(vec![], 1).unwrap();
    Arc::new(AdminState {
        cache,
        router_swap: Arc::new(ArcSwap::new(Arc::new(initial_router))),
        plugin_registry: Arc::new(PluginRegistry::new()),
        config_changed: Arc::new(Notify::new()),
    })
}

fn json_put(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(Method::PUT)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get_req(uri: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn delete_req(uri: &str) -> Request<Body> {
    Request::builder()
        .method(Method::DELETE)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// ── Health ────────────────────────────────────────────────────

#[tokio::test]
async fn health_check_returns_200() {
    let app = build_admin_router(make_state());
    let resp = app.oneshot(get_req("/apisix/admin/health")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Routes ───────────────────────────────────────────────────

#[tokio::test]
async fn put_route_creates_and_returns_200() {
    let app = build_admin_router(make_state());
    let body = serde_json::json!({
        "uri": "/test",
        "status": 1,
        "upstream": { "nodes": { "127.0.0.1:8080": 1 }, "type": "roundrobin" }
    });
    let resp = app.oneshot(json_put("/apisix/admin/routes/r1", body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = body_json(resp).await;
    assert_eq!(j["id"], "r1");
}

#[tokio::test]
async fn get_route_returns_route_after_put() {
    let state = make_state();
    let app1 = build_admin_router(Arc::clone(&state));
    let body = serde_json::json!({ "uri": "/hello", "status": 1 });
    app1.oneshot(json_put("/apisix/admin/routes/r-hello", body)).await.unwrap();

    let app2 = build_admin_router(Arc::clone(&state));
    let resp = app2.oneshot(get_req("/apisix/admin/routes/r-hello")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = body_json(resp).await;
    assert_eq!(j["id"], "r-hello");
    assert_eq!(j["uri"], "/hello");
}

#[tokio::test]
async fn get_route_returns_404_when_missing() {
    let app = build_admin_router(make_state());
    let resp = app.oneshot(get_req("/apisix/admin/routes/nonexistent")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn put_route_invalid_json_returns_4xx() {
    let app = build_admin_router(make_state());
    let req = Request::builder()
        .method(Method::PUT)
        .uri("/apisix/admin/routes/r1")
        .header("content-type", "application/json")
        .body(Body::from(r#"not-valid-json"#))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(resp.status().is_client_error(), "expected a 4xx for malformed JSON, got {}", resp.status());
}

#[tokio::test]
async fn delete_route_removes_it() {
    let state = make_state();
    // PUT route
    let app1 = build_admin_router(Arc::clone(&state));
    app1.oneshot(json_put("/apisix/admin/routes/r-del",
        serde_json::json!({ "uri": "/del", "status": 1 })
    )).await.unwrap();

    // DELETE route
    let app2 = build_admin_router(Arc::clone(&state));
    let resp = app2.oneshot(delete_req("/apisix/admin/routes/r-del")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // GET should now 404
    let app3 = build_admin_router(Arc::clone(&state));
    let resp = app3.oneshot(get_req("/apisix/admin/routes/r-del")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_routes_returns_empty_list() {
    let app = build_admin_router(make_state());
    let resp = app.oneshot(get_req("/apisix/admin/routes")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = body_json(resp).await;
    assert_eq!(j["total"], 0);
}

#[tokio::test]
async fn list_routes_returns_all_inserted() {
    let state = make_state();
    for id in ["r1", "r2", "r3"] {
        let app = build_admin_router(Arc::clone(&state));
        app.oneshot(json_put(
            &format!("/apisix/admin/routes/{id}"),
            serde_json::json!({ "uri": format!("/{id}"), "status": 1 }),
        )).await.unwrap();
    }
    let app = build_admin_router(Arc::clone(&state));
    let resp = app.oneshot(get_req("/apisix/admin/routes")).await.unwrap();
    let j = body_json(resp).await;
    assert_eq!(j["total"], 3);
}

// ── Upstreams ─────────────────────────────────────────────────

#[tokio::test]
async fn put_upstream_creates_and_returns_200() {
    let app = build_admin_router(make_state());
    let body = serde_json::json!({
        "nodes": { "127.0.0.1:8080": 1 },
        "type": "roundrobin"
    });
    let resp = app.oneshot(json_put("/apisix/admin/upstreams/u1", body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_upstream_returns_404_when_missing() {
    let app = build_admin_router(make_state());
    let resp = app.oneshot(get_req("/apisix/admin/upstreams/no-such")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_upstream_removes_it() {
    let state = make_state();
    let app1 = build_admin_router(Arc::clone(&state));
    app1.oneshot(json_put("/apisix/admin/upstreams/u1",
        serde_json::json!({ "nodes": { "127.0.0.1:8080": 1 }, "type": "roundrobin" })
    )).await.unwrap();

    let app2 = build_admin_router(Arc::clone(&state));
    let resp = app2.oneshot(delete_req("/apisix/admin/upstreams/u1")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let app3 = build_admin_router(Arc::clone(&state));
    let resp = app3.oneshot(get_req("/apisix/admin/upstreams/u1")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_upstreams_total_reflects_inserts() {
    let state = make_state();
    for id in ["u1", "u2"] {
        let app = build_admin_router(Arc::clone(&state));
        app.oneshot(json_put(
            &format!("/apisix/admin/upstreams/{id}"),
            serde_json::json!({ "nodes": { "127.0.0.1:8080": 1 }, "type": "roundrobin" }),
        )).await.unwrap();
    }
    let app = build_admin_router(Arc::clone(&state));
    let resp = app.oneshot(get_req("/apisix/admin/upstreams")).await.unwrap();
    let j = body_json(resp).await;
    assert_eq!(j["total"], 2);
}

// ── Consumers ─────────────────────────────────────────────────

#[tokio::test]
async fn put_consumer_creates_and_returns_200() {
    let app = build_admin_router(make_state());
    let body = serde_json::json!({
        "plugins": { "key-auth": { "key": "my-secret-key" } }
    });
    let resp = app.oneshot(json_put("/apisix/admin/consumers/alice", body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = body_json(resp).await;
    assert_eq!(j["username"], "alice");
}

#[tokio::test]
async fn get_consumer_returns_404_when_missing() {
    let app = build_admin_router(make_state());
    let resp = app.oneshot(get_req("/apisix/admin/consumers/nobody")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn put_consumer_updates_key_index() {
    let state = make_state();
    let app = build_admin_router(Arc::clone(&state));
    app.oneshot(json_put("/apisix/admin/consumers/bob",
        serde_json::json!({ "plugins": { "key-auth": { "key": "bob-key" } } })
    )).await.unwrap();

    // Consumer key index must be populated immediately
    assert_eq!(state.cache.find_consumer_by_key("bob-key"), Some("bob".to_string()));
}

#[tokio::test]
async fn delete_consumer_removes_from_key_index() {
    let state = make_state();
    let app1 = build_admin_router(Arc::clone(&state));
    app1.oneshot(json_put("/apisix/admin/consumers/carol",
        serde_json::json!({ "plugins": { "key-auth": { "key": "carol-key" } } })
    )).await.unwrap();

    let app2 = build_admin_router(Arc::clone(&state));
    app2.oneshot(delete_req("/apisix/admin/consumers/carol")).await.unwrap();

    assert!(state.cache.find_consumer_by_key("carol-key").is_none());
}

#[tokio::test]
async fn list_consumers_total_reflects_inserts() {
    let state = make_state();
    for name in ["alice", "bob"] {
        let app = build_admin_router(Arc::clone(&state));
        app.oneshot(json_put(
            &format!("/apisix/admin/consumers/{name}"),
            serde_json::json!({}),
        )).await.unwrap();
    }
    let app = build_admin_router(Arc::clone(&state));
    let resp = app.oneshot(get_req("/apisix/admin/consumers")).await.unwrap();
    let j = body_json(resp).await;
    assert_eq!(j["total"], 2);
}

// ── Plugins list ──────────────────────────────────────────────

#[tokio::test]
async fn plugins_list_returns_ok() {
    let app = build_admin_router(make_state());
    let resp = app.oneshot(get_req("/apisix/admin/plugins/list")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
