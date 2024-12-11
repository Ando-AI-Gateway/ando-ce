use crate::route::Route;
use matchit::Match;
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

/// Result of a route match.
pub struct RouteMatch {
    pub route_id: String,
    pub params: Vec<(String, String)>,
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

    /// Match a request to a route. Zero allocation on hit.
    #[inline]
    pub fn match_route<'a>(&'a self, method: &str, path: &str, _host: Option<&str>) -> Option<RouteMatch> {
        // Try method-specific tree first
        if let Some(tree) = self.method_trees.get(method) {
            if let Ok(matched) = tree.at(path) {
                let route_id = matched.value.clone();
                // Host filtering
                if let Some(route) = self.routes.get(&route_id) {
                    if !route.hosts.is_empty() {
                        if let Some(host) = _host {
                            if !route.hosts.iter().any(|h| h == host) {
                                // Host mismatch — fall through to catch-all
                            } else {
                                return Some(RouteMatch {
                                    route_id,
                                    params: extract_params(&matched),
                                });
                            }
                        }
                        // No host header but route requires host — skip
                    } else {
                        return Some(RouteMatch {
                            route_id,
                            params: extract_params(&matched),
                        });
                    }
                }
            }
        }

        // Try catch-all (any method) tree
        if let Ok(matched) = self.any_tree.at(path) {
            let route_id = matched.value.clone();
            if let Some(route) = self.routes.get(&route_id) {
                if !route.matches_method(method) {
                    return None;
                }
                if !route.hosts.is_empty() {
                    if let Some(host) = _host {
                        if !route.hosts.iter().any(|h| h == host) {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
            }
            return Some(RouteMatch {
                route_id,
                params: extract_params(&matched),
            });
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

/// Extract path parameters from a matchit match.
fn extract_params(matched: &Match<'_, '_, &String>) -> Vec<(String, String)> {
    matched
        .params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
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

        let m = router.match_route("GET", "/api/v1/users", None).unwrap();
        assert_eq!(m.route_id, "r1");

        let m = router.match_route("POST", "/api/v1/users", None).unwrap();
        assert_eq!(m.route_id, "r2");

        let m = router.match_route("GET", "/health", None).unwrap();
        assert_eq!(m.route_id, "r3");

        assert!(router.match_route("GET", "/not/found", None).is_none());
    }

    #[test]
    fn test_wildcard_routing() {
        let routes = vec![
            make_route("r1", "/api/*", vec!["GET"]),
        ];
        let router = Router::build(routes, 1).unwrap();

        let m = router.match_route("GET", "/api/v1/anything", None).unwrap();
        assert_eq!(m.route_id, "r1");
    }

    #[test]
    fn test_disabled_route_skipped() {
        let mut route = make_route("r1", "/test", vec!["GET"]);
        route.status = 0;
        let router = Router::build(vec![route], 1).unwrap();
        assert!(router.match_route("GET", "/test", None).is_none());
    }
}
