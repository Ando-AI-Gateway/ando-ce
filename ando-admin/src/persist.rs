//! File-based persistence for standalone mode.
//!
//! On every write (PUT/DELETE route, upstream, consumer) the current in-memory
//! state is serialized to a JSON file.  On startup the file is loaded back into
//! the ConfigCache so data survives restarts.
//!
//! The file is written atomically: first to a `.tmp` sibling, then renamed
//! over the final path, so a crash mid-write never corrupts the stored state.
//!
//! The implementation is a no-op when `AdminState::state_file` is `None`
//! (e.g. in unit tests that build an `AdminState` without specifying a path).

use crate::server::AdminState;
use ando_core::consumer::Consumer;
use ando_core::route::Route;
use ando_core::service::Service;
use ando_core::upstream::Upstream;
use ando_store::cache::ConfigCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// The shape serialized to / deserialized from the state file.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PersistedState {
    #[serde(default)]
    pub routes: HashMap<String, Route>,
    #[serde(default)]
    pub services: HashMap<String, Service>,
    #[serde(default)]
    pub upstreams: HashMap<String, Upstream>,
    #[serde(default)]
    pub consumers: HashMap<String, Consumer>,
}

/// Save the current `ConfigCache` contents to `state.state_file`.
///
/// Returns immediately (no-op) if `state_file` is `None`.
/// Logs a warning rather than panicking on I/O errors.
pub fn save_state(state: &AdminState) {
    let path = match &state.state_file {
        Some(p) => p.clone(),
        None => return,
    };

    // Snapshot the four maps
    let persisted = PersistedState {
        routes: state
            .cache
            .routes
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect(),
        services: state
            .cache
            .services
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect(),
        upstreams: state
            .cache
            .upstreams
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect(),
        consumers: state
            .cache
            .consumers
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect(),
    };

    // Serialize
    let json = match serde_json::to_string_pretty(&persisted) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "persist: failed to serialize state");
            return;
        }
    };

    // Ensure parent directory exists
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(error = %e, dir = %parent.display(), "persist: failed to create state dir");
        return;
    }

    // Atomic write: tmp file → rename
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&tmp, &json) {
        tracing::warn!(error = %e, path = %tmp.display(), "persist: failed to write tmp file");
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, &path) {
        tracing::warn!(error = %e, "persist: failed to rename tmp → state file");
        return;
    }

    tracing::debug!(path = %path.display(), "persist: state saved");
}

/// Load a previously saved state file into `cache`.
///
/// * If the file does not exist            → silently returns (first run).
/// * If the file exists but is malformed   → logs a warning and returns.
/// * On success                            → cache is populated and the consumer
///   key index is rebuilt.
pub fn load_state(path: &Path, cache: &ConfigCache) {
    if !path.exists() {
        tracing::debug!(path = %path.display(), "persist: no state file found, starting fresh");
        return;
    }

    let data = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "persist: failed to read state file");
            return;
        }
    };

    let persisted: PersistedState = match serde_json::from_str(&data) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "persist: state file is malformed, ignoring");
            return;
        }
    };

    let routes_count = persisted.routes.len();
    let services_count = persisted.services.len();
    let upstreams_count = persisted.upstreams.len();
    let consumers_count = persisted.consumers.len();

    for (k, v) in persisted.routes {
        cache.routes.insert(k, v);
    }
    for (k, v) in persisted.services {
        cache.services.insert(k, v);
    }
    for (k, v) in persisted.upstreams {
        cache.upstreams.insert(k, v);
    }
    for (k, v) in persisted.consumers {
        cache.consumers.insert(k, v);
    }
    cache.rebuild_consumer_key_index();

    tracing::info!(
        routes = routes_count,
        services = services_count,
        upstreams = upstreams_count,
        consumers = consumers_count,
        path = %path.display(),
        "persist: state restored from file"
    );
}

/// Convenience wrapper that accepts `Option<Arc<std::path::PathBuf>>` or similar.
/// Used internally — call `save_state(state)` directly from handlers.
pub fn load_state_opt(path: Option<&Path>, cache: &ConfigCache) {
    if let Some(p) = path {
        load_state(p, cache);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ando_core::route::Route;
    use ando_core::upstream::Upstream;
    use tempfile::tempdir;

    fn make_route(id: &str) -> Route {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "uri": "/test",
            "upstream_id": "u1"
        }))
        .unwrap()
    }

    fn make_upstream(id: &str) -> Upstream {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "nodes": {"127.0.0.1:8080": 1}
        }))
        .unwrap()
    }

    #[test]
    fn round_trip_routes_and_upstreams() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        let cache = ConfigCache::new();

        cache.routes.insert("r1".to_string(), make_route("r1"));
        cache
            .upstreams
            .insert("u1".to_string(), make_upstream("u1"));

        // Persist to file by constructing PersistedState directly
        let persisted = PersistedState {
            routes: cache
                .routes
                .iter()
                .map(|e| (e.key().clone(), e.value().clone()))
                .collect(),
            upstreams: cache
                .upstreams
                .iter()
                .map(|e| (e.key().clone(), e.value().clone()))
                .collect(),
            consumers: Default::default(),
        };
        let json = serde_json::to_string_pretty(&persisted).unwrap();
        std::fs::write(&path, &json).unwrap();

        // Load into a fresh cache
        let cache2 = ConfigCache::new();
        load_state(&path, &cache2);

        assert!(cache2.routes.contains_key("r1"));
        assert!(cache2.upstreams.contains_key("u1"));
    }

    #[test]
    fn load_missing_file_is_noop() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let cache = ConfigCache::new();
        // Should not panic
        load_state(&path, &cache);
        assert_eq!(cache.routes.len(), 0);
    }

    #[test]
    fn load_malformed_file_is_noop() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not valid json {{{{").unwrap();
        let cache = ConfigCache::new();
        load_state(&path, &cache);
        assert_eq!(cache.routes.len(), 0);
    }
}
