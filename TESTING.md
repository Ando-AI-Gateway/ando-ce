# Testing Guide — Ando CE

This document is the authoritative reference for reaching production-grade test
coverage. Work through each section in order; the proxy and store layers are the
highest priority because they are on every hot path.

---

## Current State

| Crate | Unit tests | Integration | Status |
|---|---|---|---|
| `ando-core` (error, route, router, upstream, consumer) | ~40 | none | ✅ baseline |
| `ando-plugin` (plugin, pipeline, registry) | ~21 | none | ✅ baseline |
| `ando-plugins` (key_auth only) | ~12 | none | ⚠️ incomplete |
| `ando-proxy` | **0** | **0** | ❌ critical gap |
| `ando-admin` | **0** | **0** | ❌ critical gap |
| `ando-store` | **0** | 0 | ❌ critical gap |
| `ando-observability` | **0** | 0 | ❌ missing |

---

## 1. Add dev-dependencies

Add to each crate's `Cargo.toml` that needs testing:

```toml
[dev-dependencies]
# Async test runtime (tokio is fine for tests — monoio not needed)
tokio = { version = "1", features = ["full", "test-util"] }
# HTTP client for integration tests
reqwest = { version = "0.12", features = ["json"] }
# Property-based testing
proptest = "1"
# Mock HTTP server
wiremock = "0.6"
# Fake data
fake = { version = "2", features = ["derive"] }
```

Add to workspace `Cargo.toml` under `[workspace.dependencies]`:

```toml
tokio        = { version = "1", features = ["full"] }
wiremock     = "0.6"
proptest     = "1"
```

---

## 2. `ando-store` — ConfigCache

File: `ando-store/src/cache.rs`

Add a `#[cfg(test)] mod tests` block at the bottom covering:

### 2.1 Consumer key index

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ando_core::consumer::Consumer;
    use std::collections::HashMap;

    fn consumer_with_key(username: &str, key: &str) -> Consumer {
        let mut plugins = HashMap::new();
        plugins.insert(
            "key-auth".to_string(),
            serde_json::json!({ "key": key }),
        );
        Consumer { username: username.to_string(), plugins, ..Default::default() }
    }

    #[test]
    fn find_consumer_by_key_returns_username() {
        let cache = ConfigCache::new();
        cache.consumers.insert("alice".to_string(), consumer_with_key("alice", "secret-123"));
        cache.rebuild_consumer_key_index();
        assert_eq!(cache.find_consumer_by_key("secret-123"), Some("alice".to_string()));
    }

    #[test]
    fn find_consumer_by_key_returns_none_for_unknown_key() {
        let cache = ConfigCache::new();
        cache.rebuild_consumer_key_index();
        assert!(cache.find_consumer_by_key("bad-key").is_none());
    }

    #[test]
    fn rebuild_consumer_key_index_replaces_stale_entries() {
        let cache = ConfigCache::new();
        cache.consumers.insert("alice".to_string(), consumer_with_key("alice", "old-key"));
        cache.rebuild_consumer_key_index();

        // Replace consumer with a new key
        cache.consumers.insert("alice".to_string(), consumer_with_key("alice", "new-key"));
        cache.rebuild_consumer_key_index();

        assert!(cache.find_consumer_by_key("old-key").is_none());
        assert_eq!(cache.find_consumer_by_key("new-key"), Some("alice".to_string()));
    }

    #[test]
    fn all_routes_returns_all_inserted_routes() {
        let cache = ConfigCache::new();
        let route_a = serde_json::from_value::<ando_core::route::Route>(
            serde_json::json!({ "id": "r1", "uri": "/a", "status": 1 })
        ).unwrap();
        let route_b = serde_json::from_value::<ando_core::route::Route>(
            serde_json::json!({ "id": "r2", "uri": "/b", "status": 1 })
        ).unwrap();
        cache.routes.insert("r1".to_string(), route_a);
        cache.routes.insert("r2".to_string(), route_b);
        assert_eq!(cache.all_routes().len(), 2);
    }
}
```

---

## 3. `ando-proxy` — ProxyWorker (unit)

File: `ando-proxy/src/proxy.rs`

`ProxyWorker::handle_request` is pure (no I/O) and fully unit-testable. Add a
`#[cfg(test)] mod tests` block covering every `RequestResult` variant.

### 3.1 Helper builder

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ando_core::config::GatewayConfig;
    use ando_core::route::Route;
    use ando_core::router::Router;
    use ando_plugin::registry::PluginRegistry;
    use ando_store::cache::ConfigCache;
    use std::sync::Arc;

    fn make_worker(routes: Vec<Route>) -> ProxyWorker {
        let router = Arc::new(Router::build(routes, 1).unwrap());
        let registry = Arc::new(PluginRegistry::new());
        let cache = ConfigCache::new();
        let config = Arc::new(GatewayConfig::default());
        ProxyWorker::new(router, registry, cache, config)
    }

    fn simple_route(id: &str, uri: &str, upstream_addr: &str) -> Route {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "uri": uri,
            "status": 1,
            "upstream": {
                "nodes": { upstream_addr: 1 },
                "type": "roundrobin"
            }
        })).unwrap()
    }
```

### 3.2 Test cases

```rust
    // ── Route matching ───────────────────────────────────────────

    #[test]
    fn returns_404_when_no_route_matches() {
        let mut w = make_worker(vec![simple_route("r1", "/api", "127.0.0.1:8080")]);
        let result = w.handle_request("GET", "/not-found", None, &[], "1.2.3.4");
        assert!(matches!(result, RequestResult::Static(RESP_404)));
    }

    #[test]
    fn returns_proxy_for_matched_route_without_plugins() {
        let mut w = make_worker(vec![simple_route("r1", "/api", "127.0.0.1:8080")]);
        let result = w.handle_request("GET", "/api", None, &[], "1.2.3.4");
        match result {
            RequestResult::Proxy { upstream_addr } => {
                assert_eq!(upstream_addr, Some("127.0.0.1:8080".to_string()));
            }
            other => panic!("Expected Proxy, got {:?}", other),
        }
    }

    #[test]
    fn disabled_route_not_matched() {
        let route: Route = serde_json::from_value(serde_json::json!({
            "id": "r1", "uri": "/disabled", "status": 0,
            "upstream": { "nodes": { "127.0.0.1:8080": 1 }, "type": "roundrobin" }
        })).unwrap();
        let mut w = make_worker(vec![route]);
        let result = w.handle_request("GET", "/disabled", None, &[], "1.2.3.4");
        assert!(matches!(result, RequestResult::Static(RESP_404)));
    }

    // ── key-auth plugin pipeline ─────────────────────────────────

    #[test]
    fn returns_401_when_key_auth_key_missing_from_consumer_store() {
        // Route has key-auth plugin but consumer store is empty
        let route: Route = serde_json::from_value(serde_json::json!({
            "id": "r1", "uri": "/secure", "status": 1,
            "plugins": { "key-auth": {} },
            "upstream": { "nodes": { "127.0.0.1:8080": 1 }, "type": "roundrobin" }
        })).unwrap();

        // Register key-auth plugin in registry
        let router = Arc::new(Router::build(vec![route], 1).unwrap());
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(ando_plugins::auth::KeyAuthPlugin));
        let cache = ConfigCache::new();
        let config = Arc::new(GatewayConfig::default());
        let mut w = ProxyWorker::new(Arc::new(router), Arc::new(registry), cache, config);

        // Request with a key that doesn't exist in consumer store
        let result = w.handle_request(
            "GET", "/secure", None,
            &[("apikey", "invalid-key")],
            "1.2.3.4"
        );
        assert!(matches!(result, RequestResult::Static(RESP_401_INVALID)));
    }

    // ── Version-triggered snapshot refresh ───────────────────────

    #[test]
    fn maybe_update_router_updates_on_version_change() {
        let routes = vec![simple_route("r1", "/api", "127.0.0.1:8080")];
        let mut w = make_worker(routes);
        let old_version = w.router_version;

        let new_route = simple_route("r2", "/v2", "127.0.0.1:9090");
        let new_router = Arc::new(Router::build(vec![new_route], old_version + 1).unwrap());
        w.maybe_update_router(new_router);

        assert_eq!(w.router_version, old_version + 1);
    }
}
```

---

## 4. `ando-admin` — HTTP handlers

File: Create `ando-admin/tests/routes_api.rs`

Use `axum::test::TestClient` (or `tower::ServiceExt`) — no real server needed.

```rust
// ando-admin/tests/routes_api.rs
use ando_admin::server::build_router;   // expose pub fn build_router() -> axum::Router
use ando_store::cache::ConfigCache;
use axum::http::StatusCode;
use tower::ServiceExt; // for .oneshot()
use axum::body::Body;
use http::Request;

fn test_app() -> axum::Router {
    build_router(/* AdminState with empty ConfigCache */)
}

#[tokio::test]
async fn put_route_creates_route() {
    let app = test_app();
    let body = serde_json::json!({
        "uri": "/test",
        "status": 1,
        "upstream": { "nodes": { "127.0.0.1:8080": 1 }, "type": "roundrobin" }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/apisix/admin/routes/r1")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_route_returns_404_when_missing() {
    let app = test_app();
    let response = app
        .oneshot(Request::builder().uri("/apisix/admin/routes/nonexistent").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_route_removes_route() {
    let app = test_app();
    // PUT then DELETE then GET
    // ... (seed then assert 404 after delete)
}

#[tokio::test]
async fn put_route_with_invalid_json_returns_400() {
    let app = test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/apisix/admin/routes/r1")
                .header("content-type", "application/json")
                .body(Body::from(r#"not-json"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

Repeat the same pattern for `upstreams`, `consumers`, and `plugins` handlers.

**Required coverage per resource type:**

| Scenario | Routes | Upstreams | Consumers |
|---|---|---|---|
| PUT (create) | ✅ | ✅ | ✅ |
| PUT (update existing) | ✅ | ✅ | ✅ |
| GET (exists) | ✅ | ✅ | ✅ |
| GET (missing) 404 | ✅ | ✅ | ✅ |
| DELETE (exists) | ✅ | ✅ | ✅ |
| DELETE (missing) | ✅ | ✅ | ✅ |
| LIST | ✅ | ✅ | ✅ |
| Invalid JSON body | ✅ | ✅ | ✅ |

---

## 5. `ando-plugins` — All auth plugins

File: `ando-plugins/src/auth/<plugin>.rs`

Every plugin must have its own `#[cfg(test)] mod tests` covering at least:

### 5.1 `jwt-auth`

```rust
#[test]
fn valid_token_sets_consumer_vars() { /* sign a token, verify ctx.vars */ }

#[test]
fn expired_token_returns_401() { /* use exp = 0 */ }

#[test]
fn missing_authorization_header_returns_401() { /* no header */ }

#[test]
fn wrong_algorithm_returns_401() { /* sign RS256, configure HS256 */ }
```

### 5.2 `basic-auth`

```rust
#[test]
fn valid_base64_credentials_sets_consumer() { /* base64("user:pass") */ }

#[test]
fn invalid_password_returns_401() { /* wrong pass */ }

#[test]
fn missing_authorization_header_returns_401() {}

#[test]
fn malformed_base64_returns_401() { /* "Basic !!!" */ }
```

### 5.3 `rate-limiting`

```rust
#[test]
fn first_requests_within_limit_pass() { /* n < limit → Continue */ }

#[test]
fn request_exceeding_limit_returns_429() { /* run limit+1 times */ }

#[test]
fn window_reset_allows_new_requests() { /* advance mock time */ }
```

Use `std::time::Instant` injection (pass a `now: Instant` parameter) or wrap
the time source in a trait so tests can substitute a fake clock.

### 5.4 `ip-restriction`

```rust
#[test]
fn allowed_ip_passes() { /* 192.168.1.1 in allowlist */ }

#[test]
fn blocked_ip_returns_403() { /* 10.0.0.1 in denylist */ }

#[test]
fn cidr_allowlist_matches_subnet() { /* 192.168.0.0/24 */ }

#[test]
fn cidr_denylist_blocks_subnet() {}
```

### 5.5 `cors`

```rust
#[test]
fn preflight_returns_correct_headers() { /* OPTIONS → 204 with CORS headers */ }

#[test]
fn disallowed_origin_returns_403() {}

#[test]
fn wildcard_origin_passes_all() {}
```

---

## 6. Integration tests — proxy end-to-end

Create `ando-ce/tests/proxy_integration.rs`.

These tests start a real monoio proxy listener on a random port and a `wiremock`
upstream, then make real HTTP requests through the proxy.

```rust
// tests/proxy_integration.rs
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

/// Starts a proxy pointing at a wiremock upstream.
/// Returns the proxy's listen address.
async fn start_proxy_for_upstream(upstream_addr: &str) -> String {
    // 1. Build ConfigCache with one route pointing at upstream_addr
    // 2. Build PluginRegistry
    // 3. Spawn proxy on 127.0.0.1:0 (OS picks port)
    // 4. Return the bound address
    todo!()
}

#[tokio::test]
async fn proxy_forwards_request_to_upstream() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/hello"))
        .respond_with(ResponseTemplate::new(200).set_body_string("world"))
        .mount(&mock_server)
        .await;

    let proxy_addr = start_proxy_for_upstream(&mock_server.address().to_string()).await;
    let resp = reqwest::get(format!("http://{}/hello", proxy_addr)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "world");
}

#[tokio::test]
async fn proxy_returns_404_for_unmatched_path() {
    let mock_server = MockServer::start().await;
    let proxy_addr = start_proxy_for_upstream(&mock_server.address().to_string()).await;

    let resp = reqwest::get(format!("http://{}/no-such-route", proxy_addr)).await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn proxy_returns_502_when_upstream_unreachable() {
    // Point proxy at an address with nothing listening
    let proxy_addr = start_proxy_for_upstream("127.0.0.1:19999").await;
    let resp = reqwest::get(format!("http://{}/api", proxy_addr)).await.unwrap();
    assert_eq!(resp.status(), 502);
}

#[tokio::test]
async fn key_auth_blocks_request_with_missing_key() {
    // Route with key-auth plugin, no key in headers
    todo!()
}

#[tokio::test]
async fn key_auth_allows_request_with_valid_consumer_key() {
    // Consumer inserted into cache, valid apikey header
    todo!()
}

#[tokio::test]
async fn hot_config_reload_takes_effect_without_restart() {
    // Add route → verify pass; add second route via admin API → verify pass
    todo!()
}
```

---

## 7. `ando-observability` — unit tests

File: `ando-observability/src/access_log.rs`

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn access_log_format_includes_required_fields() {
        // Call format_log_entry(method, path, status, latency_ms)
        // Assert JSON output has: method, path, status, latency_ms, timestamp
    }

    #[test]
    fn access_log_sanitizes_path_with_query_string() { }
}
```

File: `ando-observability/src/metrics.rs`

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn request_counter_increments() {
        let m = Metrics::new();
        m.record_request("GET", "/api", 200, 5.0);
        assert_eq!(m.total_requests(), 1);
    }

    #[test]
    fn latency_histogram_records_value() { }
}
```

---

## 8. Property-based tests

Add to `ando-core/src/router.rs` tests using `proptest`:

```rust
proptest::proptest! {
    #[test]
    fn router_never_panics_on_arbitrary_method_and_path(
        method in "[A-Z]{1,10}",
        path in "/[a-z/]{0,50}",
    ) {
        let router = Router::build(vec![], 1).unwrap();
        let _ = router.match_route(&method, &path, None);
    }
}
```

---

## 9. CI pipeline

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main, "ce/*", "feat/*"]
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache cargo registry
        uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Clippy (deny warnings)
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: Run tests
        run: cargo test --workspace --all-features

      - name: Coverage (optional, requires cargo-llvm-cov)
        run: |
          cargo install cargo-llvm-cov --locked
          cargo llvm-cov --workspace --lcov --output-path lcov.info
          # Fail if coverage < 70%
          cargo llvm-cov report --fail-under-lines 70
```

---

## 10. Coverage targets (minimum before production release)

| Area | Minimum line coverage |
|---|---|
| `ando-core` | 80% |
| `ando-plugin` | 80% |
| `ando-plugins` (every plugin) | 75% |
| `ando-store` (ConfigCache) | 80% |
| `ando-proxy` (ProxyWorker logic) | 70% |
| `ando-admin` (all handlers) | 75% |
| Integration (end-to-end proxy) | all happy-path + 5 error scenarios |

Run locally:

```bash
cargo install cargo-llvm-cov --locked
cargo llvm-cov --workspace --html --open
```

---

## 11. Implementation order

Work through this checklist in sequence — each step unblocks the next.

- [x] **Step 1** — `ando-store`: `ConfigCache` unit tests (Section 2)
- [x] **Step 2** — `ando-proxy`: `ProxyWorker::handle_request` unit tests (Section 3)
- [x] **Step 3** — `ando-admin`: expose `build_router()`, add handler tests (Section 4)
- [x] **Step 4** — `ando-plugins`: add tests for every remaining plugin (Section 5)
- [x] **Step 5** — `ando-observability`: access log + metrics unit tests (Section 7)
- [x] **Step 6** — Integration test scaffold + at least 6 passing scenarios (Section 6)
- [x] **Step 7** — Property-based tests for router (Section 8)
- [x] **Step 8** — CI workflow with coverage gate (Section 9)
- [ ] **Step 9** — Meet all coverage targets (Section 10)
