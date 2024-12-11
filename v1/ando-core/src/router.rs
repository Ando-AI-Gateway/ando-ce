use crate::route::Route;
use dashmap::DashMap;
use matchit::Router as MatchitRouter;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{info, warn};

/// Thread-safe, high-performance router using a radix trie.
///
/// This router supports dynamic route registration and provides
/// O(log n) lookup time using a radix tree (via `matchit` crate).
///
/// Optimisation: host constraints are pre-compiled into the `CompiledRouter`
/// so `match_route` never touches the `routes` DashMap on the hot path.
pub struct Router {
    /// Current compiled route tree
    inner: arc_swap::ArcSwap<CompiledRouter>,

    /// Source of truth: all registered routes by ID
    routes: DashMap<String, Route>,

    /// Monotonically-increasing version, bumped on every rebuild.
    /// Used by the proxy to invalidate its pipeline cache.
    version: AtomicU64,
}

/// Pre-compiled host constraints for a route (stored inside the compiled router).
#[derive(Clone, Debug)]
struct HostConstraint {
    /// Exact hosts (no wildcards).
    exact: Vec<String>,
    /// Wildcard suffixes (e.g. `*.example.com` → `.example.com`).
    wildcard_suffixes: Vec<String>,
}

impl HostConstraint {
    fn from_route(route: &Route) -> Self {
        let hosts = route.all_hosts();
        let mut exact = Vec::new();
        let mut wildcard_suffixes = Vec::new();
        for h in hosts {
            if h.starts_with('*') {
                wildcard_suffixes.push(h[1..].to_string());
            } else {
                exact.push(h.to_string());
            }
        }
        Self {
            exact,
            wildcard_suffixes,
        }
    }

    /// Returns `true` if there are no host constraints (route matches any host).
    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.exact.is_empty() && self.wildcard_suffixes.is_empty()
    }

    /// Check if the given `request_host` satisfies this constraint.
    #[inline]
    fn matches(&self, request_host: Option<&str>) -> bool {
        if self.is_empty() {
            return true;
        }
        let Some(host) = request_host else {
            return false;
        };
        let host = host.split(':').next().unwrap_or(host);
        for h in &self.exact {
            if h == host {
                return true;
            }
        }
        for suffix in &self.wildcard_suffixes {
            if host.ends_with(suffix.as_str()) {
                return true;
            }
        }
        false
    }
}

struct CompiledRouter {
    /// Method-specific routers for faster matching
    method_routers: std::collections::HashMap<String, MatchitRouter<Arc<str>>>,

    /// Catch-all router (for routes with no method constraint)
    any_method_router: MatchitRouter<Arc<str>>,

    /// Pre-compiled host constraints indexed by route_id.
    host_constraints: std::collections::HashMap<Arc<str>, HostConstraint>,
}

/// Result of a route match.
#[derive(Debug)]
pub struct RouteMatch {
    /// The matched route ID — Arc<str> avoids a heap allocation per request
    /// (just an atomic increment to clone the Arc from the compiled router).
    pub route_id: Arc<str>,

    /// Extracted path parameters
    pub params: Vec<(String, String)>,
}

impl Router {
    pub fn new() -> Self {
        let compiled = CompiledRouter {
            method_routers: std::collections::HashMap::new(),
            any_method_router: MatchitRouter::new(),
            host_constraints: std::collections::HashMap::new(),
        };

        Self {
            inner: arc_swap::ArcSwap::new(Arc::new(compiled)),
            routes: DashMap::new(),
            version: AtomicU64::new(0),
        }
    }

    /// Returns the current route table version.
    #[inline]
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
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
    #[inline]
    pub fn get_route(&self, route_id: &str) -> Option<Route> {
        self.routes.get(route_id).map(|r| r.clone())
    }

    /// Get all routes.
    pub fn all_routes(&self) -> Vec<Route> {
        self.routes.iter().map(|r| r.value().clone()).collect()
    }

    /// Match an incoming request against registered routes.
    ///
    /// Hot-path optimised: uses an `arc_swap::Guard` (not `Arc::clone`),
    /// and checks pre-compiled host constraints without touching the DashMap.
    #[inline]
    pub fn match_route(&self, method: &str, path: &str, host: Option<&str>) -> Option<RouteMatch> {
        let compiled = self.inner.load();

        // First try method-specific router
        if let Some(method_router) = compiled.method_routers.get(method) {
            if let Ok(matched) = method_router.at(path) {
                let route_id = matched.value;
                if self.check_host_fast(&compiled, route_id, host) {
                    let params = if matched.params.is_empty() {
                        Vec::new()
                    } else {
                        matched
                            .params
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect()
                    };
                    return Some(RouteMatch {
                        route_id: Arc::clone(route_id),
                        params,
                    });
                }
            }
        }

        // Fall back to any-method router
        if let Ok(matched) = compiled.any_method_router.at(path) {
            let route_id = matched.value;
            if self.check_host_fast(&compiled, route_id, host) {
                let params = if matched.params.is_empty() {
                    Vec::new()
                } else {
                    matched
                        .params
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect()
                };
                return Some(RouteMatch {
                    route_id: Arc::clone(route_id),
                    params,
                });
            }
        }

        None
    }

    /// Fast host constraint check using pre-compiled data in CompiledRouter
    /// (no DashMap lookup).
    #[inline(always)]
    fn check_host_fast(
        &self,
        compiled: &CompiledRouter,
        route_id: &str,
        request_host: Option<&str>,
    ) -> bool {
        match compiled.host_constraints.get(route_id) {
            Some(hc) => hc.matches(request_host),
            None => true, // No constraint entry → matches everything
        }
    }

    /// Rebuild the compiled router from current routes.
    ///
    /// This is called automatically by `add_route` / `remove_route`, but
    /// can also be called manually after a batch of concurrent mutations
    /// to guarantee the compiled tree is fully up-to-date.
    pub fn rebuild(&self) -> anyhow::Result<()> {
        let mut method_routers: std::collections::HashMap<String, MatchitRouter<Arc<str>>> =
            std::collections::HashMap::new();
        let mut any_method_router = MatchitRouter::new();
        let mut host_constraints: std::collections::HashMap<Arc<str>, HostConstraint> =
            std::collections::HashMap::new();

        // Sort routes by priority (higher first)
        let mut routes: Vec<Route> = self.routes.iter().map(|r| r.value().clone()).collect();
        routes.sort_by(|a, b| b.priority.cmp(&a.priority));

        for route in &routes {
            if !route.is_active() {
                continue;
            }

            let route_id_arc: Arc<str> = Arc::from(route.id.as_str());

            // Pre-compile host constraints
            let hc = HostConstraint::from_route(route);
            if !hc.is_empty() {
                host_constraints.insert(Arc::clone(&route_id_arc), hc);
            }

            for uri in route.all_uris() {
                if route.methods.is_empty() {
                    if let Err(e) = any_method_router.insert(uri, Arc::clone(&route_id_arc)) {
                        warn!(route_id = %route.id, uri = %uri, error = %e, "Failed to insert route into any-method router");
                    }
                } else {
                    for method in &route.methods {
                        let router = method_routers
                            .entry(method.as_str().to_string())
                            .or_insert_with(MatchitRouter::new);
                        if let Err(e) = router.insert(uri, Arc::clone(&route_id_arc)) {
                            warn!(route_id = %route.id, uri = %uri, method = ?method, error = %e, "Failed to insert route");
                        }
                    }
                }
            }
        }

        let compiled = CompiledRouter {
            method_routers,
            any_method_router,
            host_constraints,
        };

        self.inner.store(Arc::new(compiled));
        self.version.fetch_add(1, Ordering::Release);
        info!(count = routes.len(), "Router rebuilt");
        Ok(())
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
        assert_eq!(m.unwrap().route_id.as_ref(), "r1");

        let m = router.match_route("POST", "/api/users", None);
        assert!(m.is_some());
        assert_eq!(m.unwrap().route_id.as_ref(), "r2");

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
        assert_eq!(m.route_id.as_ref(), "r1");
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
