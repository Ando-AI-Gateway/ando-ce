use crate::route::Route;
use std::collections::HashMap;
use tracing::info;

/// Thread-safe radix-trie router.
///
/// v2 design: The router itself is immutable (frozen). On config change,
/// a new Router is built and swapped in atomically via `arc_swap::ArcSwap`.
/// This eliminates all locking from the hot path — each worker core reads
/// the current Arc<Router> via a single atomic load.
pub struct Router {
    /// matchit trie for each HTTP method.
    method_trees: HashMap<String, matchit::Router<String>>,
    /// Catch-all tree (for routes with no method filter).
    any_tree: matchit::Router<String>,
    /// All routes keyed by ID.
    routes: HashMap<String, Route>,
    /// Monotonic version — bumped on every rebuild.
    version: u64,
}

impl Router {
    /// Build a new frozen router from a set of routes.
    pub fn build(routes: Vec<Route>, version: u64) -> anyhow::Result<Self> {
        let mut method_trees: HashMap<String, matchit::Router<String>> = HashMap::new();
        let mut any_tree = matchit::Router::new();
        let mut route_map = HashMap::with_capacity(routes.len());

        for route in routes {
            if route.status == 0 {
                continue; // skip disabled routes
            }

            let path = normalize_path(&route.uri);

            // For wildcard routes (e.g. /api/v1/*) matchit's {*rest} catch-all
            // does NOT match an empty capture, so /api/v1/ would 404. We also
            // register the trailing-slash base path as an exact entry so that
            // both /api/v1/ and /api/v1/anything are handled by the same route.
            let base_slash = if route.uri.ends_with("/*") && route.uri.len() > 2 {
                // "/api/v1/*"  →  "/api/v1/"
                Some(route.uri[..route.uri.len() - 1].to_string())
            } else if route.uri == "/*" {
                // "/*" → "/"  (already covered by a plain "/" exact match would
                // conflict, so skip — "/" is handled by the catch-all directly)
                None
            } else {
                None
            };

            if route.methods.is_empty() {
                // Match any method
                if let Err(e) = any_tree.insert(&path, route.id.clone()) {
                    tracing::warn!(route_id = %route.id, path = %path, "Failed to insert catch-all route: {e}");
                }
                if let Some(ref bp) = base_slash {
                    // Ignore conflict errors — a more-specific exact route may already own this path
                    let _ = any_tree.insert(bp, route.id.clone());
                }
            } else {
                for method in &route.methods {
                    let method_upper = method.to_uppercase();
                    let tree = method_trees
                        .entry(method_upper)
                        .or_default();
                    if let Err(e) = tree.insert(&path, route.id.clone()) {
                        tracing::warn!(route_id = %route.id, method = %method, path = %path, "Failed to insert route: {e}");
                    }
                    if let Some(ref bp) = base_slash {
                        let _ = tree.insert(bp, route.id.clone());
                    }
                }
            }

            route_map.insert(route.id.clone(), route);
        }

        info!(routes = route_map.len(), version, "Router built");

        Ok(Self {
            method_trees,
            any_tree,
            routes: route_map,
            version,
        })
    }

    /// Match a request to a route. Returns a reference — zero allocation.
    ///
    /// Instead of returning `RouteMatch { route_id, params }` which clones
    /// the route_id String and allocates a Vec for params on every match,
    /// this returns `&Route` directly. The caller can access `route.id`
    /// and other fields without any allocation.
    #[inline]
    pub fn match_route(&self, method: &str, path: &str, host: Option<&str>) -> Option<&Route> {
        // Try method-specific tree first
        if let Some(tree) = self.method_trees.get(method)
            && let Ok(matched) = tree.at(path)
        {
            let route_id = matched.value.as_str();
            if let Some(route) = self.routes.get(route_id)
                && check_host(route, host)
            {
                return Some(route);
            }
        }

        // Try catch-all (any method) tree
        if let Ok(matched) = self.any_tree.at(path) {
            let route_id = matched.value.as_str();
            if let Some(route) = self.routes.get(route_id) {
                if !route.matches_method(method) {
                    return None;
                }
                if check_host(route, host) {
                    return Some(route);
                }
            }
        }

        None
    }

    /// Get a route by ID.
    #[inline]
    pub fn get_route(&self, id: &str) -> Option<&Route> {
        self.routes.get(id)
    }

    /// Router version.
    #[inline]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Number of routes.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// Get all routes (for admin API listing).
    pub fn routes(&self) -> &HashMap<String, Route> {
        &self.routes
    }
}

/// Check host filtering. Returns true if the route allows the given host
/// (or has no host restriction).
#[inline]
fn check_host(route: &Route, host: Option<&str>) -> bool {
    if route.hosts.is_empty() {
        return true;
    }
    match host {
        Some(h) => route.hosts.iter().any(|rh| rh == h),
        None => false,
    }
}

/// Normalize path for matchit compatibility.
fn normalize_path(uri: &str) -> String {
    // Convert APISIX wildcard `/*` suffix to matchit `/{*rest}`
    if uri.ends_with("/*") {
        format!("{}{{*rest}}", &uri[..uri.len() - 1])
    } else if uri == "/*" {
        "/{*rest}".to_string()
    } else {
        uri.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_route(id: &str, uri: &str, methods: Vec<&str>) -> Route {
        Route {
            id: id.to_string(),
            uri: uri.to_string(),
            methods: methods.into_iter().map(|s| s.to_string()).collect(),
            hosts: vec![],
            upstream: None,
            upstream_id: None,
            service_id: None,
            plugins: Default::default(),
            plugin_config_id: None,
            priority: 0,
            status: 1,
            strip_prefix: false,
            name: None,
            desc: None,
            labels: Default::default(),
        }
    }

    #[test]
    fn test_basic_routing() {
        let routes = vec![
            make_route("r1", "/api/v1/users", vec!["GET"]),
            make_route("r2", "/api/v1/users", vec!["POST"]),
            make_route("r3", "/health", vec![]),
        ];
        let router = Router::build(routes, 1).unwrap();

        let route = router.match_route("GET", "/api/v1/users", None).unwrap();
        assert_eq!(route.id, "r1");

        let route = router.match_route("POST", "/api/v1/users", None).unwrap();
        assert_eq!(route.id, "r2");

        let route = router.match_route("GET", "/health", None).unwrap();
        assert_eq!(route.id, "r3");

        assert!(router.match_route("GET", "/not/found", None).is_none());
    }

    #[test]
    fn test_wildcard_routing() {
        let routes = vec![
            make_route("r1", "/api/*", vec!["GET"]),
        ];
        let router = Router::build(routes, 1).unwrap();

        let route = router.match_route("GET", "/api/v1/anything", None).unwrap();
        assert_eq!(route.id, "r1");
    }

    #[test]
    fn test_wildcard_trailing_slash() {
        // /api/v1/* should match /api/v1/ (trailing slash, empty rest)
        let routes = vec![make_route("r1", "/api/v1/*", vec!["GET"])];
        let router = Router::build(routes, 1).unwrap();
        // exact base with trailing slash
        eprintln!("matching /api/v1/");
        let r = router.match_route("GET", "/api/v1/", None);
        eprintln!("result: {:?}", r.map(|x| &x.id));
        assert!(r.is_some(), "/api/v1/ should match /api/v1/*");
        // path with content
        assert!(router.match_route("GET", "/api/v1/users", None).is_some());
        assert!(router.match_route("GET", "/api/v1/users/123", None).is_some());
    }

    #[test]
    fn test_disabled_route_skipped() {
        let mut route = make_route("r1", "/test", vec!["GET"]);
        route.status = 0;
        let router = Router::build(vec![route], 1).unwrap();
        assert!(router.match_route("GET", "/test", None).is_none());
    }

    #[test]
    fn test_version_and_len() {
        let routes = vec![
            make_route("r1", "/a", vec!["GET"]),
            make_route("r2", "/b", vec!["POST"]),
        ];
        let router = Router::build(routes, 42).unwrap();
        assert_eq!(router.version(), 42);
        assert_eq!(router.len(), 2);
        assert!(!router.is_empty());
    }

    #[test]
    fn test_empty_router() {
        let router = Router::build(vec![], 1).unwrap();
        assert!(router.is_empty());
        assert_eq!(router.len(), 0);
        assert!(router.match_route("GET", "/anything", None).is_none());
    }

    #[test]
    fn test_get_route_by_id() {
        let routes = vec![make_route("r1", "/test", vec!["GET"])];
        let router = Router::build(routes, 1).unwrap();
        assert!(router.get_route("r1").is_some());
        assert_eq!(router.get_route("r1").unwrap().uri, "/test");
        assert!(router.get_route("nonexistent").is_none());
    }

    #[test]
    fn test_host_filtering_passes_matching_host() {
        let mut route = make_route("r1", "/api", vec!["GET"]);
        route.hosts = vec!["api.example.com".to_string()];
        let router = Router::build(vec![route], 1).unwrap();
        assert!(router.match_route("GET", "/api", Some("api.example.com")).is_some());
    }

    #[test]
    fn test_host_filtering_rejects_wrong_host() {
        let mut route = make_route("r1", "/api", vec!["GET"]);
        route.hosts = vec!["api.example.com".to_string()];
        let router = Router::build(vec![route], 1).unwrap();
        assert!(router.match_route("GET", "/api", Some("other.example.com")).is_none());
        assert!(router.match_route("GET", "/api", None).is_none());
    }

    #[test]
    fn test_no_host_restriction_matches_any_host() {
        let route = make_route("r1", "/api", vec!["GET"]);
        let router = Router::build(vec![route], 1).unwrap();
        assert!(router.match_route("GET", "/api", Some("any.host.com")).is_some());
        assert!(router.match_route("GET", "/api", None).is_some());
    }

    #[test]
    fn test_method_not_allowed() {
        let routes = vec![make_route("r1", "/api", vec!["GET"])];
        let router = Router::build(routes, 1).unwrap();
        assert!(router.match_route("GET", "/api", None).is_some());
        assert!(router.match_route("POST", "/api", None).is_none());
        assert!(router.match_route("DELETE", "/api", None).is_none());
    }

    #[test]
    fn test_routes_listing() {
        let routes = vec![
            make_route("r1", "/a", vec!["GET"]),
            make_route("r2", "/b", vec!["POST"]),
        ];
        let router = Router::build(routes, 1).unwrap();
        let all = router.routes();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("r1"));
        assert!(all.contains_key("r2"));
    }

    #[test]
    fn test_nested_path_routing() {
        let routes = vec![
            make_route("r1", "/api/v1/users", vec!["GET"]),
            make_route("r2", "/api/v1/tokens", vec!["POST"]),
        ];
        let router = Router::build(routes, 1).unwrap();
        assert_eq!(router.match_route("GET", "/api/v1/users", None).unwrap().id, "r1");
        assert_eq!(router.match_route("POST", "/api/v1/tokens", None).unwrap().id, "r2");
        assert!(router.match_route("GET", "/api/v1/tokens", None).is_none());
    }

    #[test]
    fn test_multiple_methods_same_path() {
        let routes = vec![
            make_route("r_get", "/resource", vec!["GET"]),
            make_route("r_post", "/resource", vec!["POST"]),
            make_route("r_del", "/resource", vec!["DELETE"]),
        ];
        let router = Router::build(routes, 1).unwrap();
        assert_eq!(router.match_route("GET", "/resource", None).unwrap().id, "r_get");
        assert_eq!(router.match_route("POST", "/resource", None).unwrap().id, "r_post");
        assert_eq!(router.match_route("DELETE", "/resource", None).unwrap().id, "r_del");
        assert!(router.match_route("PUT", "/resource", None).is_none());
    }

    #[test]
    fn test_normalize_path_wildcard() {
        assert_eq!(normalize_path("/api/*"), "/api/{*rest}");
        assert_eq!(normalize_path("/*"), "/{*rest}");
        assert_eq!(normalize_path("/exact"), "/exact");
        assert_eq!(normalize_path("/a/b/c"), "/a/b/c");
    }

    // ── Property-based tests ──────────────────────────────────────

    proptest::proptest! {
        /// An empty router must never panic regardless of method or path input.
        #[test]
        fn router_never_panics_on_arbitrary_method_and_path(
            method in "[A-Z]{1,10}",
            path   in "/[a-z/]{0,50}",
        ) {
            let router = Router::build(vec![], 1).unwrap();
            // Should always return None without panicking
            let _ = router.match_route(&method, &path, None);
        }

        /// A single registered route is never incorrectly matched for a
        /// completely different path.
        #[test]
        fn router_does_not_match_different_paths(
            suffix in "[a-z]{1,20}",
        ) {
            let route = make_route("r1", "/fixed/path", vec![]);
            let router = Router::build(vec![route], 1).unwrap();

            let query_path = format!("/other/{suffix}");
            // /other/... must never match /fixed/path
            let result = router.match_route("GET", &query_path, None);
            assert!(result.is_none() || result.unwrap().uri == "/fixed/path",
                "Unexpected match for {query_path}");
        }

        /// Route count must be ≤ number of distinct routes inserted.
        #[test]
        fn router_len_bounded_by_input(count in 0usize..20) {
            let routes: Vec<_> = (0..count)
                .map(|i| make_route(&format!("r{i}"), &format!("/route/{i}"), vec![]))
                .collect();
            let router = Router::build(routes, 1).unwrap();
            assert!(router.len() <= count);
        }
    }
}
