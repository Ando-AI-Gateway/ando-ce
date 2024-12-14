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

            if route.methods.is_empty() {
                // Match any method
                if let Err(e) = any_tree.insert(&path, route.id.clone()) {
                    tracing::warn!(route_id = %route.id, path = %path, "Failed to insert catch-all route: {e}");
                }
            } else {
                for method in &route.methods {
                    let method_upper = method.to_uppercase();
                    let tree = method_trees
                        .entry(method_upper)
                        .or_insert_with(matchit::Router::new);
                    if let Err(e) = tree.insert(&path, route.id.clone()) {
                        tracing::warn!(route_id = %route.id, method = %method, path = %path, "Failed to insert route: {e}");
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
        if let Some(tree) = self.method_trees.get(method) {
            if let Ok(matched) = tree.at(path) {
                let route_id = matched.value.as_str();
                if let Some(route) = self.routes.get(route_id) {
                    if check_host(route, host) {
                        return Some(route);
                    }
                }
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
    fn test_disabled_route_skipped() {
        let mut route = make_route("r1", "/test", vec!["GET"]);
        route.status = 0;
        let router = Router::build(vec![route], 1).unwrap();
        assert!(router.match_route("GET", "/test", None).is_none());
    }
}
