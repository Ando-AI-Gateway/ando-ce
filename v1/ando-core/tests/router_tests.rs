use ando_core::route::{HttpMethod, Route};
use ando_core::router::Router;
use std::collections::HashMap;

// =============================================================================
// Helper Functions
// =============================================================================

fn test_route(id: &str, uri: &str, methods: Vec<HttpMethod>) -> Route {
    Route {
        id: id.to_string(),
        name: id.to_string(),
        description: String::new(),
        uri: uri.to_string(),
        uris: vec![],
        methods,
        host: None,
        hosts: vec![],
        remote_addrs: vec![],
        vars: vec![],
        priority: 0,
        enable: true,
        upstream: None,
        upstream_id: None,
        service_id: None,
        plugins: HashMap::new(),
        plugin_config_id: None,
        labels: HashMap::new(),
        status: 1,
        timeout: None,
        created_at: None,
        updated_at: None,
    }
}


fn test_route_with_host(id: &str, uri: &str, host: &str) -> Route {
    let mut route = test_route(id, uri, vec![]);
    route.host = Some(host.to_string());
    route
}

// =============================================================================
// Basic Router Tests
// =============================================================================

#[test]
fn test_router_new() {
    let router = Router::new();
    assert_eq!(router.route_count(), 0);
}

#[test]
fn test_router_default() {
    let router = Router::default();
    assert_eq!(router.route_count(), 0);
}

#[test]
fn test_add_single_route() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users", vec![HttpMethod::Get])).unwrap();
    assert_eq!(router.route_count(), 1);
}

#[test]
fn test_add_multiple_routes() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users", vec![HttpMethod::Get])).unwrap();
    router.add_route(test_route("r2", "/api/posts", vec![HttpMethod::Get])).unwrap();
    router.add_route(test_route("r3", "/api/comments", vec![HttpMethod::Post])).unwrap();
    assert_eq!(router.route_count(), 3);
}

#[test]
fn test_remove_route() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users", vec![HttpMethod::Get])).unwrap();
    router.add_route(test_route("r2", "/api/posts", vec![HttpMethod::Get])).unwrap();
    assert_eq!(router.route_count(), 2);

    router.remove_route("r1").unwrap();
    assert_eq!(router.route_count(), 1);
}

#[test]
fn test_remove_nonexistent_route() {
    let router = Router::new();
    // Removing a nonexistent route should not error
    router.remove_route("nonexistent").unwrap();
    assert_eq!(router.route_count(), 0);
}

#[test]
fn test_get_route() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users", vec![HttpMethod::Get])).unwrap();

    let route = router.get_route("r1");
    assert!(route.is_some());
    assert_eq!(route.unwrap().uri, "/api/users");

    let route = router.get_route("r2");
    assert!(route.is_none());
}

#[test]
fn test_all_routes() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users", vec![])).unwrap();
    router.add_route(test_route("r2", "/api/posts", vec![])).unwrap();

    let all = router.all_routes();
    assert_eq!(all.len(), 2);
}

// =============================================================================
// Route Matching Tests
// =============================================================================

#[test]
fn test_match_exact_path() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users", vec![HttpMethod::Get])).unwrap();

    let m = router.match_route("GET", "/api/users", None);
    assert!(m.is_some());
    assert_eq!(m.unwrap().route_id.as_ref(), "r1");
}

#[test]
fn test_no_match_wrong_path() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users", vec![HttpMethod::Get])).unwrap();

    let m = router.match_route("GET", "/api/posts", None);
    assert!(m.is_none());
}

#[test]
fn test_match_method_specific() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users", vec![HttpMethod::Get])).unwrap();
    router.add_route(test_route("r2", "/api/users", vec![HttpMethod::Post])).unwrap();

    let m = router.match_route("GET", "/api/users", None);
    assert!(m.is_some());
    assert_eq!(m.unwrap().route_id.as_ref(), "r1");

    let m = router.match_route("POST", "/api/users", None);
    assert!(m.is_some());
    assert_eq!(m.unwrap().route_id.as_ref(), "r2");
}

#[test]
fn test_no_match_wrong_method() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users", vec![HttpMethod::Get])).unwrap();

    let m = router.match_route("DELETE", "/api/users", None);
    assert!(m.is_none());
}

#[test]
fn test_match_any_method() {
    let router = Router::new();
    // Empty methods means match any method
    router.add_route(test_route("r1", "/api/catch-all", vec![])).unwrap();

    assert!(router.match_route("GET", "/api/catch-all", None).is_some());
    assert!(router.match_route("POST", "/api/catch-all", None).is_some());
    assert!(router.match_route("PUT", "/api/catch-all", None).is_some());
    assert!(router.match_route("DELETE", "/api/catch-all", None).is_some());
}

// =============================================================================
// Parametric Route Tests
// =============================================================================

#[test]
fn test_match_parametric_route() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/users/{id}", vec![])).unwrap();

    let m = router.match_route("GET", "/api/users/123", None).unwrap();
    assert_eq!(m.route_id.as_ref(), "r1");
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0], ("id".to_string(), "123".to_string()));
}

#[test]
fn test_match_multiple_params() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/{version}/users/{id}", vec![])).unwrap();

    let m = router.match_route("GET", "/api/v2/users/456", None).unwrap();
    assert_eq!(m.route_id.as_ref(), "r1");
    assert_eq!(m.params.len(), 2);
    // matchit returns params in order
    assert!(m.params.iter().any(|(k, v)| k == "version" && v == "v2"));
    assert!(m.params.iter().any(|(k, v)| k == "id" && v == "456"));
}

// =============================================================================
// Host Constraint Tests
// =============================================================================

#[test]
fn test_match_exact_host() {
    let router = Router::new();
    router.add_route(test_route_with_host("r1", "/api", "example.com")).unwrap();

    let m = router.match_route("GET", "/api", Some("example.com"));
    assert!(m.is_some());
}

#[test]
fn test_no_match_wrong_host() {
    let router = Router::new();
    router.add_route(test_route_with_host("r1", "/api", "example.com")).unwrap();

    let m = router.match_route("GET", "/api", Some("other.com"));
    assert!(m.is_none());
}

#[test]
fn test_match_wildcard_host() {
    let router = Router::new();
    router.add_route(test_route_with_host("r1", "/api", "*.example.com")).unwrap();

    assert!(router.match_route("GET", "/api", Some("foo.example.com")).is_some());
    assert!(router.match_route("GET", "/api", Some("bar.example.com")).is_some());
    assert!(router.match_route("GET", "/api", Some("other.com")).is_none());
}

#[test]
fn test_match_host_strips_port() {
    let router = Router::new();
    router.add_route(test_route_with_host("r1", "/api", "example.com")).unwrap();

    // Host header with port should still match
    let m = router.match_route("GET", "/api", Some("example.com:8080"));
    assert!(m.is_some());
}

#[test]
fn test_no_host_constraint_matches_any() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api", vec![])).unwrap();

    // No host constraint on route should match any host or no host
    assert!(router.match_route("GET", "/api", None).is_some());
    assert!(router.match_route("GET", "/api", Some("anything.com")).is_some());
}

#[test]
fn test_host_required_but_not_provided() {
    let router = Router::new();
    router.add_route(test_route_with_host("r1", "/api", "example.com")).unwrap();

    // Route requires a host, but request doesn't have one
    let m = router.match_route("GET", "/api", None);
    assert!(m.is_none());
}

#[test]
fn test_multiple_hosts() {
    let router = Router::new();
    let mut route = test_route("r1", "/api", vec![]);
    route.host = Some("primary.com".to_string());
    route.hosts = vec!["secondary.com".to_string(), "tertiary.com".to_string()];
    router.add_route(route).unwrap();

    assert!(router.match_route("GET", "/api", Some("primary.com")).is_some());
    assert!(router.match_route("GET", "/api", Some("secondary.com")).is_some());
    assert!(router.match_route("GET", "/api", Some("tertiary.com")).is_some());
    assert!(router.match_route("GET", "/api", Some("other.com")).is_none());
}

// =============================================================================
// Inactive Route Tests
// =============================================================================

#[test]
fn test_inactive_route_not_matched() {
    let router = Router::new();
    let mut route = test_route("r1", "/api/disabled", vec![]);
    route.enable = false;
    router.add_route(route).unwrap();

    let m = router.match_route("GET", "/api/disabled", None);
    assert!(m.is_none());
}

#[test]
fn test_status_zero_route_not_matched() {
    let router = Router::new();
    let mut route = test_route("r1", "/api/disabled", vec![]);
    route.status = 0;
    router.add_route(route).unwrap();

    let m = router.match_route("GET", "/api/disabled", None);
    assert!(m.is_none());
}

// =============================================================================
// Replace All Tests
// =============================================================================

#[test]
fn test_replace_all_routes() {
    let router = Router::new();
    router.add_route(test_route("r1", "/old1", vec![])).unwrap();
    router.add_route(test_route("r2", "/old2", vec![])).unwrap();
    assert_eq!(router.route_count(), 2);

    let new_routes = vec![
        test_route("r3", "/new1", vec![]),
        test_route("r4", "/new2", vec![]),
        test_route("r5", "/new3", vec![]),
    ];
    router.replace_all(new_routes).unwrap();

    assert_eq!(router.route_count(), 3);
    assert!(router.get_route("r1").is_none());
    assert!(router.get_route("r2").is_none());
    assert!(router.get_route("r3").is_some());
    assert!(router.get_route("r4").is_some());
    assert!(router.get_route("r5").is_some());

    // Matching should work with new routes
    assert!(router.match_route("GET", "/new1", None).is_some());
    assert!(router.match_route("GET", "/old1", None).is_none());
}

#[test]
fn test_replace_all_empty() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api", vec![])).unwrap();
    assert_eq!(router.route_count(), 1);

    router.replace_all(vec![]).unwrap();
    assert_eq!(router.route_count(), 0);
    assert!(router.match_route("GET", "/api", None).is_none());
}

// =============================================================================
// Route Update Tests
// =============================================================================

#[test]
fn test_update_existing_route() {
    let router = Router::new();
    router.add_route(test_route("r1", "/api/v1", vec![HttpMethod::Get])).unwrap();

    // Update the route with a new path
    router.add_route(test_route("r1", "/api/v2", vec![HttpMethod::Get])).unwrap();

    assert_eq!(router.route_count(), 1);
    let route = router.get_route("r1").unwrap();
    assert_eq!(route.uri, "/api/v2");

    // Should match new path, not old
    assert!(router.match_route("GET", "/api/v2", None).is_some());
}

// =============================================================================
// Concurrency Tests
// =============================================================================

#[test]
fn test_router_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let router = Arc::new(Router::new());

    // Add routes from multiple threads
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let router = Arc::clone(&router);
            thread::spawn(move || {
                let uri = format!("/api/thread{}", i);
                router
                    .add_route(test_route(&format!("r{}", i), &uri, vec![]))
                    .unwrap();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Force a final rebuild to ensure the compiled router reflects
    // all concurrent insertions (each add_route triggers rebuild, but
    // the last one to store() may not see the very last insertion).
    router.rebuild().unwrap();

    assert_eq!(router.route_count(), 10);

    // All routes should be matchable
    for i in 0..10 {
        let uri = format!("/api/thread{}", i);
        assert!(
            router.match_route("GET", &uri, None).is_some(),
            "Route /api/thread{} should be matchable",
            i
        );
    }
}
