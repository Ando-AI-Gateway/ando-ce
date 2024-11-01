use crate::route::Route;
use dashmap::DashMap;
use matchit::Router as MatchitRouter;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Thread-safe, high-performance router using a radix trie.
///
/// This router supports dynamic route registration and provides
/// O(log n) lookup time using a radix tree (via `matchit` crate).
pub struct Router {
    /// Current compiled route tree
    inner: arc_swap::ArcSwap<CompiledRouter>,

    /// Source of truth: all registered routes by ID
    routes: DashMap<String, Route>,
}

struct CompiledRouter {
    /// Method-specific routers for faster matching
    method_routers: std::collections::HashMap<String, MatchitRouter<String>>,

    /// Catch-all router (for routes with no method constraint)
    any_method_router: MatchitRouter<String>,
}

/// Result of a route match.
#[derive(Debug)]
pub struct RouteMatch {
    /// The matched route ID
    pub route_id: String,

    /// Extracted path parameters
    pub params: Vec<(String, String)>,
}

impl Router {
    pub fn new() -> Self {
        let compiled = CompiledRouter {
            method_routers: std::collections::HashMap::new(),
            any_method_router: MatchitRouter::new(),
        };

        Self {
            inner: arc_swap::ArcSwap::new(Arc::new(compiled)),
            routes: DashMap::new(),
        }
    }

    /// Add or update a route. Triggers recompilation of the route tree.
    pub fn add_route(&self, route: Route) -> anyhow::Result<()> {
        info!(route_id = %route.id, uri = %route.uri, "Adding route");
        self.routes.insert(route.id.clone(), route);
        self.rebuild()
    }

    /// Remove a route by ID. Triggers recompilation.
    pub fn remove_route(&self, route_id: &str) -> anyhow::Result<()> {
        info!(route_id = %route_id, "Removing route");
        self.routes.remove(route_id);
        self.rebuild()
    }

    /// Get a route by ID.
    pub fn get_route(&self, route_id: &str) -> Option<Route> {
        self.routes.get(route_id).map(|r| r.clone())
    }

    /// Get all routes.
    pub fn all_routes(&self) -> Vec<Route> {
        self.routes.iter().map(|r| r.value().clone()).collect()
    }

    /// Match an incoming request against registered routes.
    pub fn match_route(&self, method: &str, path: &str, host: Option<&str>) -> Option<RouteMatch> {
        let compiled = self.inner.load();

        // First try method-specific router
        if let Some(method_router) = compiled.method_routers.get(method) {
            if let Ok(matched) = method_router.at(path) {
                let route_id = matched.value.clone();
                // Verify host constraint if set
                if self.check_host_constraint(&route_id, host) {
                    let params = matched
                        .params
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    debug!(route_id = %route_id, method = %method, path = %path, "Route matched (method-specific)");
                    return Some(RouteMatch {
                        route_id,
                        params,
                    });
                }
            }
        }

        // Fall back to any-method router
        if let Ok(matched) = compiled.any_method_router.at(path) {
            let route_id = matched.value.clone();
            if self.check_host_constraint(&route_id, host) {
                let params = matched
                    .params
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                debug!(route_id = %route_id, method = %method, path = %path, "Route matched (any-method)");
                return Some(RouteMatch {
                    route_id,
                    params,
                });
            }
        }

        debug!(method = %method, path = %path, "No route matched");
        None
    }

    /// Rebuild the compiled router from current routes.
    fn rebuild(&self) -> anyhow::Result<()> {
        let mut method_routers: std::collections::HashMap<String, MatchitRouter<String>> =
            std::collections::HashMap::new();
        let mut any_method_router = MatchitRouter::new();

        // Sort routes by priority (higher first)
        let mut routes: Vec<Route> = self.routes.iter().map(|r| r.value().clone()).collect();
        routes.sort_by(|a, b| b.priority.cmp(&a.priority));

        for route in &routes {
            if !route.is_active() {
                continue;
            }

            for uri in route.all_uris() {
                if route.methods.is_empty() {
                    // No method constraint — add to catch-all
                    if let Err(e) = any_method_router.insert(uri, route.id.clone()) {
                        warn!(route_id = %route.id, uri = %uri, error = %e, "Failed to insert route into any-method router");
                    }
                } else {
                    // Add to each method-specific router
                    for method in &route.methods {
                        let router = method_routers
                            .entry(method.as_str().to_string())
                            .or_insert_with(MatchitRouter::new);
                        if let Err(e) = router.insert(uri, route.id.clone()) {
                            warn!(route_id = %route.id, uri = %uri, method = ?method, error = %e, "Failed to insert route");
                        }
                    }
                }
            }
        }

        let compiled = CompiledRouter {
            method_routers,
            any_method_router,
        };

        self.inner.store(Arc::new(compiled));
        info!(count = routes.len(), "Router rebuilt");
        Ok(())
    }

    /// Check if the matched route's host constraint is satisfied.
    fn check_host_constraint(&self, route_id: &str, request_host: Option<&str>) -> bool {
        let Some(route) = self.routes.get(route_id) else {
            return false;
        };

        let hosts = route.all_hosts();
        if hosts.is_empty() {
            return true; // No host constraint
        }

        let Some(host) = request_host else {
            return false; // Host required but not provided
        };

        // Strip port from host
        let host = host.split(':').next().unwrap_or(host);

        hosts.iter().any(|h| {
            if h.starts_with('*') {
                // Wildcard match: *.example.com
                let suffix = &h[1..];
                host.ends_with(suffix)
            } else {
                *h == host
            }
        })
    }

    /// Get the total number of registered routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Replace all routes atomically (used during config sync).
    pub fn replace_all(&self, routes: Vec<Route>) -> anyhow::Result<()> {
        self.routes.clear();
        for route in routes {
            self.routes.insert(route.id.clone(), route);
        }
        self.rebuild()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{HttpMethod, Route};

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
            plugins: std::collections::HashMap::new(),
            plugin_config_id: None,
            labels: std::collections::HashMap::new(),
            status: 1,
            timeout: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_basic_route_matching() {
        let router = Router::new();
        router
            .add_route(test_route("r1", "/api/users", vec![HttpMethod::Get]))
            .unwrap();
        router
            .add_route(test_route(
                "r2",
                "/api/users",
                vec![HttpMethod::Post],
            ))
            .unwrap();

        let m = router.match_route("GET", "/api/users", None);
        assert!(m.is_some());
        assert_eq!(m.unwrap().route_id, "r1");

        let m = router.match_route("POST", "/api/users", None);
        assert!(m.is_some());
        assert_eq!(m.unwrap().route_id, "r2");

        let m = router.match_route("DELETE", "/api/users", None);
        assert!(m.is_none());
    }

    #[test]
    fn test_parametric_route() {
        let router = Router::new();
        router
            .add_route(test_route("r1", "/api/users/{id}", vec![]))
            .unwrap();

        let m = router.match_route("GET", "/api/users/123", None).unwrap();
        assert_eq!(m.route_id, "r1");
        assert_eq!(m.params[0], ("id".to_string(), "123".to_string()));
    }

    #[test]
    fn test_wildcard_host_matching() {
        let router = Router::new();
        let mut route = test_route("r1", "/api", vec![]);
        route.host = Some("*.example.com".to_string());
        router.add_route(route).unwrap();

        let m = router.match_route("GET", "/api", Some("foo.example.com"));
        assert!(m.is_some());

        let m = router.match_route("GET", "/api", Some("other.com"));
        assert!(m.is_none());
    }
}
