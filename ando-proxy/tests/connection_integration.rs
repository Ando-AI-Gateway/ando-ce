/// End-to-end tests for `handle_connection` using a real monoio runtime and
/// real TCP sockets — no network mocking needed.
///
/// These tests exercise the I/O dispatch loop in connection.rs that cannot
/// be covered by unit tests alone (monoio async I/O is not compatible with
/// tokio's `#[tokio::test]`).
use ando_core::config::GatewayConfig;
use ando_core::router::Router;
use ando_plugin::registry::PluginRegistry;
use ando_proxy::connection::handle_connection;
use ando_proxy::proxy::{ConnPool, ProxyWorker};
use ando_store::cache::ConfigCache;
use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

fn make_rt() -> monoio::Runtime<monoio::LegacyDriver> {
    monoio::RuntimeBuilder::<monoio::LegacyDriver>::new()
        .build()
        .expect("monoio runtime build failed")
}

fn make_worker(routes: Vec<serde_json::Value>) -> ProxyWorker {
    let parsed: Vec<ando_core::route::Route> = routes
        .into_iter()
        .map(|v| serde_json::from_value(v).unwrap())
        .collect();
    let router = Arc::new(Router::build(parsed, 1).unwrap());
    let registry = Arc::new(PluginRegistry::new());
    let cache = ConfigCache::new();
    let config = Arc::new(GatewayConfig::default());
    ProxyWorker::new(router, registry, cache, config)
}

/// Extract the HTTP status line from the first line of a raw response.
fn status_line(buf: &[u8]) -> &str {
    let s = std::str::from_utf8(buf).unwrap_or("");
    s.lines().next().unwrap_or("")
}

// ── Test 1: no route → 404 ─────────────────────────────────────────────────

#[test]
fn handle_connection_404_no_matching_route() {
    make_rt().block_on(async {
        let listener = monoio::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_addr = listener.local_addr().unwrap();

        let proxy = Rc::new(RefCell::new(make_worker(vec![])));
        let pool = Rc::new(RefCell::new(ConnPool::new(0)));

        monoio::spawn(async move {
            if let Ok((stream, peer)) = listener.accept().await {
                let _ = handle_connection(stream, peer, proxy, pool).await;
            }
        });

        let mut client = monoio::net::TcpStream::connect(proxy_addr.to_string().as_str())
            .await
            .unwrap();
        let (_, _) = client
            .write_all(b"GET /missing HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n".to_vec())
            .await;

        let buf = vec![0u8; 512];
        let (n, buf) = client.read(buf).await;
        let n = n.unwrap_or(0);
        let first = status_line(&buf[..n]);
        assert!(first.contains("404"), "Expected 404, got: {first:?}");
    });
}

// ── Test 2: invalid HTTP → 400 ────────────────────────────────────────────

#[test]
fn handle_connection_400_for_malformed_request() {
    make_rt().block_on(async {
        let listener = monoio::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_addr = listener.local_addr().unwrap();

        let proxy = Rc::new(RefCell::new(make_worker(vec![])));
        let pool = Rc::new(RefCell::new(ConnPool::new(0)));

        monoio::spawn(async move {
            if let Ok((stream, peer)) = listener.accept().await {
                let _ = handle_connection(stream, peer, proxy, pool).await;
            }
        });

        let mut client = monoio::net::TcpStream::connect(proxy_addr.to_string().as_str())
            .await
            .unwrap();
        // Malformed request — not a valid HTTP request line
        let (_, _) = client.write_all(b"NOTHTTP GARBAGE\r\n\r\n".to_vec()).await;

        let buf = vec![0u8; 512];
        let (n, buf) = client.read(buf).await;
        let n = n.unwrap_or(0);
        let first = status_line(&buf[..n]);
        assert!(first.contains("400"), "Expected 400, got: {first:?}");
    });
}

// ── Test 3: unreachable upstream → 502 ────────────────────────────────────

#[test]
fn handle_connection_502_upstream_unreachable() {
    // Grab a free port synchronously before entering the async runtime
    let tmp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_port = tmp.local_addr().unwrap().port();
    drop(tmp);

    make_rt().block_on(async {
        let route = serde_json::json!({
            "id": "r502",
            "uri": "/dead",
            "status": 1,
            "upstream": {
                "nodes": { format!("127.0.0.1:{dead_port}"): 1 },
                "type": "roundrobin"
            }
        });

        let listener = monoio::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_addr = listener.local_addr().unwrap();

        let proxy = Rc::new(RefCell::new(make_worker(vec![route])));
        let pool = Rc::new(RefCell::new(ConnPool::new(0)));

        monoio::spawn(async move {
            if let Ok((stream, peer)) = listener.accept().await {
                let _ = handle_connection(stream, peer, proxy, pool).await;
            }
        });

        let mut client = monoio::net::TcpStream::connect(proxy_addr.to_string().as_str())
            .await
            .unwrap();
        let (_, _) = client
            .write_all(b"GET /dead HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n".to_vec())
            .await;

        let buf = vec![0u8; 512];
        let (n, buf) = client.read(buf).await;
        let n = n.unwrap_or(0);
        let first = status_line(&buf[..n]);
        assert!(first.contains("502"), "Expected 502, got: {first:?}");
    });
}

// ── Test 4: plugin response (key-auth → 401) ───────────────────────────────

#[test]
fn handle_connection_plugin_response_key_auth_blocks_missing_key() {
    make_rt().block_on(async {
        let route = serde_json::json!({
            "id": "r-secure",
            "uri": "/secure",
            "status": 1,
            "plugins": { "key-auth": {} },
            "upstream": {
                "nodes": { "127.0.0.1:9999": 1 },
                "type": "roundrobin"
            }
        });

        let parsed: ando_core::route::Route = serde_json::from_value(route).unwrap();
        let router = Arc::new(Router::build(vec![parsed], 1).unwrap());
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(ando_plugins::auth::key_auth::KeyAuthPlugin));
        let cache = ConfigCache::new();
        let config = Arc::new(GatewayConfig::default());
        let worker = ProxyWorker::new(router, Arc::new(registry), cache, config);

        let listener = monoio::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_addr = listener.local_addr().unwrap();

        let proxy = Rc::new(RefCell::new(worker));
        let pool = Rc::new(RefCell::new(ConnPool::new(0)));

        monoio::spawn(async move {
            if let Ok((stream, peer)) = listener.accept().await {
                let _ = handle_connection(stream, peer, proxy, pool).await;
            }
        });

        let mut client = monoio::net::TcpStream::connect(proxy_addr.to_string().as_str())
            .await
            .unwrap();
        // No apikey header — key-auth should block with 401
        let (_, _) = client
            .write_all(
                b"GET /secure HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n".to_vec(),
            )
            .await;

        let buf = vec![0u8; 512];
        let (n, buf) = client.read(buf).await;
        let n = n.unwrap_or(0);
        let first = status_line(&buf[..n]);
        assert!(first.contains("401"), "Expected 401 from key-auth, got: {first:?}");
    });
}

// ── Test 5: static response passes through correctly ──────────────────────

#[test]
fn handle_connection_static_405_for_method_not_allowed() {
    make_rt().block_on(async {
        // Route configured for GET only
        let route = serde_json::json!({
            "id": "r-get",
            "uri": "/get-only",
            "status": 1,
            "methods": ["GET"],
            "upstream": {
                "nodes": { "127.0.0.1:9999": 1 },
                "type": "roundrobin"
            }
        });

        let listener = monoio::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_addr = listener.local_addr().unwrap();

        let proxy = Rc::new(RefCell::new(make_worker(vec![route])));
        let pool = Rc::new(RefCell::new(ConnPool::new(0)));

        monoio::spawn(async move {
            if let Ok((stream, peer)) = listener.accept().await {
                let _ = handle_connection(stream, peer, proxy, pool).await;
            }
        });

        let mut client = monoio::net::TcpStream::connect(proxy_addr.to_string().as_str())
            .await
            .unwrap();
        // Send DELETE — should not match the GET-only route → 404
        let (_, _) = client
            .write_all(
                b"DELETE /get-only HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n"
                    .to_vec(),
            )
            .await;

        let buf = vec![0u8; 512];
        let (n, buf) = client.read(buf).await;
        let n = n.unwrap_or(0);
        let first = status_line(&buf[..n]);
        // Method mismatch → no route match → 404
        assert!(
            first.contains("404") || first.contains("405"),
            "Expected 404/405, got: {first:?}"
        );
    });
}
