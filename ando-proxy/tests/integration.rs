//! Integration tests: ConfigCache → Router → ProxyWorker pipeline
//!
//! These tests exercise the full data-plane dispatch path without a
//! real TCP listener. They verify that:
//!
//! 1. Routes written to the store are visible through the router.
//! 2. Consumer key lookup works after index rebuild.
//! 3. Disabled routes are not routed.
//! 4. Router version increments on each rebuild.
//! 5. Plugin registry resolves all registered plugins.
//! 6. SharedState wires everything together correctly.
//! 7. Hot router swap via ArcSwap is immediately visible.
//! 8. Multiple routes dispatch to the correct route ID.

use ando_core::config::GatewayConfig;
use ando_core::router::Router;
use ando_core::route::Route;
use ando_core::upstream::Upstream;
use ando_core::consumer::Consumer;
use ando_plugin::registry::PluginRegistry;
use ando_plugins::register_all;
use ando_store::cache::ConfigCache;
use ando_proxy::worker::SharedState;
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::sync::Arc;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_route(id: &str, uri: &str) -> Route {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "uri": uri,
        "upstream_id": "up1"
    }))
    .expect("valid route JSON")
}

fn make_method_route(id: &str, uri: &str, methods: Vec<&str>) -> Route {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "uri": uri,
        "upstream_id": "up1",
        "methods": methods
    }))
    .expect("valid route JSON")
}

fn make_disabled_route(id: &str, uri: &str) -> Route {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "uri": uri,
        "upstream_id": "up1",
        "status": 0
    }))
    .expect("valid route JSON")
}

fn make_upstream(id: &str, addr: &str) -> Upstream {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "nodes": { addr: 1 }
    }))
    .expect("valid upstream JSON")
}

fn make_consumer_entry(username: &str, key: &str) -> Consumer {
    Consumer {
        username: username.to_string(),
        plugins: {
            let mut m = HashMap::new();
            m.insert("key-auth".to_string(), serde_json::json!({ "key": key }));
            m
        },
        desc: None,
        labels: HashMap::new(),
    }
}

// ── Test 1: route in cache becomes matchable via router ───────────────────────

#[test]
fn route_in_cache_is_matched_by_router() {
    let cache = ConfigCache::new();
    cache.routes.insert("r1".into(), make_route("r1", "/hello"));

    let routes = cache.all_routes();
    let router = Router::build(routes, 1).unwrap();

    let matched = router.match_route("GET", "/hello", None);
    assert!(matched.is_some(), "Route /hello should match");
    assert_eq!(matched.unwrap().id, "r1");
}

// ── Test 2: upstream in cache is retrievable ──────────────────────────────────

#[test]
fn upstream_in_cache_is_retrievable() {
    let cache = ConfigCache::new();
    cache
        .upstreams
        .insert("up1".into(), make_upstream("up1", "10.0.0.1:8080"));

    let up = cache.upstreams.get("up1");
    assert!(up.is_some());
    assert_eq!(up.unwrap().id.as_deref(), Some("up1"));
}

// ── Test 3: consumer key lookup after index rebuild ───────────────────────────

#[test]
fn consumer_key_lookup_after_index_rebuild() {
    let cache = ConfigCache::new();
    cache
        .consumers
        .insert("alice".into(), make_consumer_entry("alice", "secret-key-123"));
    cache.rebuild_consumer_key_index();

    let username = cache.find_consumer_by_key("secret-key-123");
    assert_eq!(username.as_deref(), Some("alice"));
}

// ── Test 4: unknown key returns None ─────────────────────────────────────────

#[test]
fn consumer_key_unknown_returns_none() {
    let cache = ConfigCache::new();
    cache.rebuild_consumer_key_index();
    assert!(cache.find_consumer_by_key("nonexistent").is_none());
}

// ── Test 5: disabled route is not matched ────────────────────────────────────

#[test]
fn disabled_route_is_not_matched_by_router() {
    let cache = ConfigCache::new();
    cache
        .routes
        .insert("r-dis".into(), make_disabled_route("r-dis", "/off"));

    let routes = cache.all_routes();
    let router = Router::build(routes, 1).unwrap();

    assert!(
        router.match_route("GET", "/off", None).is_none(),
        "Disabled route must not match"
    );
}

// ── Test 6: router version reported correctly ─────────────────────────────────

#[test]
fn router_version_is_correct() {
    let r = make_route("r1", "/v");
    let router_v1 = Router::build(vec![r.clone()], 1).unwrap();
    let router_v3 = Router::build(vec![r], 3).unwrap();

    assert_eq!(router_v1.version(), 1);
    assert_eq!(router_v3.version(), 3);
}

// ── Test 7: plugin registry resolves all registered plugins ──────────────────

#[test]
fn plugin_registry_has_all_plugins_after_register_all() {
    let mut registry = PluginRegistry::new();
    register_all(&mut registry);

    let expected = [
        "key-auth",
        "basic-auth",
        "jwt-auth",
        "ip-restriction",
        "rate-limiting",
        "cors",
        "security-headers",
    ];
    for name in &expected {
        assert!(
            registry.get(name).is_some(),
            "Plugin '{name}' must be in registry"
        );
    }
    assert_eq!(registry.len(), expected.len());
}

// ── Test 8: SharedState wires components correctly ────────────────────────────

#[test]
fn shared_state_provides_consistent_view() {
    let cache = ConfigCache::new();
    cache.routes.insert("r1".into(), make_route("r1", "/api"));
    cache
        .upstreams
        .insert("up1".into(), make_upstream("up1", "10.0.0.1:9000"));

    let routes = cache.all_routes();
    let router = Router::build(routes, 1).unwrap();

    let mut registry = PluginRegistry::new();
    register_all(&mut registry);

    let shared = SharedState::new(router, registry, cache, GatewayConfig::default());

    // Router should match /api
    let current_router = shared.router.load();
    assert!(current_router.match_route("GET", "/api", None).is_some());

    // Cache upstream accessible
    assert!(shared.config_cache.upstreams.get("up1").is_some());
}

// ── Test 9: hot ArcSwap makes new router immediately visible ──────────────────

#[test]
fn hot_arcswap_router_swap_is_immediately_visible() {
    let router_v1 = Router::build(vec![make_route("r1", "/v1")], 1).unwrap();
    let router_v2 = Router::build(vec![make_route("r2", "/v2")], 2).unwrap();

    let swap = Arc::new(ArcSwap::new(Arc::new(router_v1)));

    assert!(swap.load().match_route("GET", "/v1", None).is_some());
    assert!(swap.load().match_route("GET", "/v2", None).is_none());

    swap.store(Arc::new(router_v2));

    assert!(swap.load().match_route("GET", "/v1", None).is_none());
    assert!(swap.load().match_route("GET", "/v2", None).is_some());
}

// ── Test 10: method-specific route only matches correct method ────────────────

#[test]
fn method_specific_route_only_matches_correct_method() {
    let cache = ConfigCache::new();
    cache.routes.insert(
        "rget".into(),
        make_method_route("rget", "/resource", vec!["GET"]),
    );

    let routes = cache.all_routes();
    let router = Router::build(routes, 1).unwrap();

    assert!(router.match_route("GET", "/resource", None).is_some());
    assert!(router.match_route("POST", "/resource", None).is_none());
    assert!(router.match_route("DELETE", "/resource", None).is_none());
}

