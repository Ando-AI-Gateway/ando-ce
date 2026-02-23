# Testing Guide — Ando CE

This document is the authoritative reference for the test suite. All 8 implementation
steps have been completed. **187 tests pass, 0 failures** across the workspace
(as of the last `cargo test --workspace` run).

---

## Current State

| Crate | Unit tests | Integration | Status |
|---|---|---|---|
| `ando-core` (error, route, router, upstream, consumer) | 43 (incl. 3 proptest) | — | ✅ done |
| `ando-plugin` (plugin, pipeline, registry) | 21 | — | ✅ done |
| `ando-plugins` (key-auth, basic-auth, jwt-auth, ip-restriction, rate-limiting, cors) | 50 | — | ✅ done |
| `ando-proxy` | 20 | 10 (pipeline integration) | ✅ done |
| `ando-admin` | 18 (handler integration via `tower::ServiceExt`) | — | ✅ done |
| `ando-store` | 11 | — | ✅ done |
| `ando-observability` | 14 | — | ✅ done |

**Total: 187 tests** — run with:

```bash
cargo test --workspace
```

---

## 1. Dev-dependencies in place

Dev-dependencies already added:

| Crate | Dev-dep added |
|---|---|
| `ando-core` | `proptest = "1"` |
| `ando-proxy` | `ando-plugins` (path dep, for plugin pipeline tests) |
| `ando-admin` | `tower = { version = "0.4", features = ["util"] }` |
| workspace | `base64 = "0.22"` |

Workspace already includes `tokio`, `reqwest`, `arc-swap`, `jsonwebtoken`, `ipnet`, `base64`.

---

## 2. `ando-store` — ConfigCache ✅

File: [`ando-store/src/cache.rs`](ando-store/src/cache.rs) — **11 tests**

| Test group | Tests |
|---|---|
| `find_consumer_by_key` | found after rebuild, unknown key, not-yet-rebuilt |
| `rebuild_consumer_key_index` | stale key replaced, multiple consumers, consumer without plugin |
| `all_routes` | empty, after insert, after remove, clone shares DashMap |
| `Default` | all maps empty |

---

## 3. `ando-proxy` — ProxyWorker (unit) ✅

File: [`ando-proxy/src/proxy.rs`](ando-proxy/src/proxy.rs) — **20 tests**

| Test group | Tests |
|---|---|
| `status_text` | all known codes + unknown fallback |
| `build_response` | status line, body, custom headers, buffer clear |
| `build_upstream_request` | format, hop-by-hop header filter, content-length |
| `handle_request` — routing | 404, fast-path proxy, disabled route, wildcard, method gate |
| `handle_request` — key-auth | missing key (plugin 401), invalid key (static 401), valid key (proxy) |
| `maybe_update_router` | no-op on same version, swaps on new version |
| `upstream_addresses` | returns inline route nodes |

---

## 4. `ando-admin` — HTTP handlers ✅

File: [`ando-admin/tests/admin_api.rs`](ando-admin/tests/admin_api.rs) — **18 tests**

Uses `tower::ServiceExt::oneshot` against a shared `Arc<AdminState>` — no real TCP port needed.
`build_admin_router()` is exposed as a `pub fn` in [`ando-admin/src/server.rs`](ando-admin/src/server.rs).

| Scenario | Routes | Upstreams | Consumers |
|---|---|---|---|
| PUT (create) → 200 | ✅ | ✅ | ✅ |
| GET (exists) | ✅ | — | — |
| GET (missing) → 404 | ✅ | ✅ | ✅ |
| DELETE removes resource | ✅ | ✅ | ✅ |
| LIST reflects inserts | ✅ | ✅ | ✅ |
| PUT updates key index | — | — | ✅ |
| DELETE removes from key index | — | — | ✅ |
| Invalid JSON body → 4xx | ✅ | — | — |
| `GET /health` → 200 | ✅ | — | — |
| `GET /plugins/list` → 200 | ✅ | — | — |

---

## 5. `ando-plugins` — All plugins ✅

All 6 plugins implemented and tested — **50 tests total**.

| Plugin | File | Tests | What's covered |
|---|---|---|---|
| `key-auth` | [`auth/key_auth.rs`](ando-plugins/src/auth/key_auth.rs) | 12 | default/custom header, hide-credentials, valid/missing/empty key |
| `basic-auth` | [`auth/basic_auth.rs`](ando-plugins/src/auth/basic_auth.rs) | 9 | missing header, Bearer scheme, invalid b64, no colon, valid creds, colons-in-password, empty password, lowercase prefix |
| `jwt-auth` | [`auth/jwt_auth.rs`](ando-plugins/src/auth/jwt_auth.rs) | 7 | missing header, valid token (sets consumer + var), expired, wrong secret, no-Bearer prefix, malformed, var set |
| `ip-restriction` | [`traffic/ip_restriction.rs`](ando-plugins/src/traffic/ip_restriction.rs) | 8 | no restrictions, denylist direct/CIDR, allowlist allow/block, denylist priority, multiple CIDRs |
| `rate-limiting` | [`traffic/rate_limiting.rs`](ando-plugins/src/traffic/rate_limiting.rs) | 6 | within limit, exceeds limit (429), independent IPs, window reset, zero limit, retry-after header |
| `cors` | [`traffic/cors.rs`](ando-plugins/src/traffic/cors.rs) | 8 | no origin header, wildcard, specific list allow/block, OPTIONS 204, CORS headers, allow-credentials, simple GET vars |

All plugins registered in [`lib.rs`](ando-plugins/src/lib.rs) `register_all()`.

---

## 6. Integration tests — pipeline end-to-end ✅

File: [`ando-proxy/tests/integration.rs`](ando-proxy/tests/integration.rs) — **10 tests**

Tests the full `ConfigCache → Router → PluginRegistry → SharedState` pipeline without a
network listener. This is faster and more reliable than network-based tests because monoio
(thread-per-core) doesn't support `tokio::test` easily.

| Test | What it verifies |
|---|---|
| `route_in_cache_is_matched_by_router` | DashMap write → Router build → match |
| `upstream_in_cache_is_retrievable` | upstream store round-trip |
| `consumer_key_lookup_after_index_rebuild` | key-auth index O(1) lookup |
| `consumer_key_unknown_returns_none` | no false positives |
| `disabled_route_is_not_matched_by_router` | `status: 0` skipped by Router::build |
| `router_version_is_correct` | version passed through to Router |
| `plugin_registry_has_all_plugins_after_register_all` | all 6 plugins registered |
| `shared_state_provides_consistent_view` | SharedState wires router + cache together |
| `hot_arcswap_router_swap_is_immediately_visible` | ArcSwap atomic swap works |
| `method_specific_route_only_matches_correct_method` | GET route rejects POST/DELETE |

---

## 7. `ando-observability` — unit tests ✅

**14 tests** across two files.

### [`access_log.rs`](ando-observability/src/access_log.rs) — 6 tests

| Test | What it verifies |
|---|---|
| `serialises_all_fields` | all `AccessLogEntry` fields present in JSON output |
| `upstream_addr_none_serialises_to_null` | `Option<String>` → JSON null |
| `roundtrip_with_upstream` | JSON serialize → deserialize preserves values |
| `roundtrip_without_upstream` | same, without upstream addr |
| `various_status_codes_serialise_correctly` | 200, 400, 404, 429, 500, 502, etc. |
| `debug_format_does_not_panic` | `{:?}` formatting |

### [`metrics.rs`](ando-observability/src/metrics.rs) — 8 tests

| Test | What it verifies |
|---|---|
| `disabled_collector_has_no_fields` | `enabled=false` → all `Option` fields are `None` |
| `disabled_collector_render_returns_empty` | disabled → empty string output |
| `disabled_collector_record_request_does_not_panic` | no-op is safe |
| `enabled_collector_has_all_fields` | `enabled=true` → counters initialised |
| `enabled_collector_render_returns_prometheus_text` | output contains metric names |
| `request_counter_increments` | counter increments on each call |
| `active_connections_gauge_can_be_incremented` | gauge inc/dec works |
| `multiple_routes_tracked_independently` | label cardinality correct |

---

## 8. Property-based tests ✅

File: [`ando-core/src/router.rs`](ando-core/src/router.rs) — **3 proptest properties**

| Property | Strategy | What it proves |
|---|---|---|
| `router_never_panics_on_arbitrary_method_and_path` | `[A-Z]{1,10}` × `/[a-z/]{0,50}` | empty router never panics for any input |
| `router_does_not_match_different_paths` | random suffix string | a fixed route `/fixed/path` never spuriously matches `/other/<random>` |
| `router_len_bounded_by_input` | `0..20` routes | `router.len() ≤` number of routes inserted |

Dep: `proptest = "1"` in `ando-core` `[dev-dependencies]`.

---

## 9. CI pipeline ✅

File: [`.github/workflows/ci.yml`](.github/workflows/ci.yml)

Two jobs:

**`test`** — runs on every push and PR:
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --all-features`

**`coverage`** — push to `main` only (requires `test` to pass):
1. `cargo llvm-cov --workspace --lcov` (via `taiki-e/install-action`)
2. Codecov upload (non-fatal)
3. `cargo llvm-cov report --fail-under-lines 70` — enforces 70% gate

Triggers: push to `main`, `ce/*`, `feat/*`; all pull requests.

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

## 11. Implementation checklist

- [x] **Step 1** — `ando-store`: 11 ConfigCache unit tests
- [x] **Step 2** — `ando-proxy`: 20 ProxyWorker unit tests
- [x] **Step 3** — `ando-admin`: `build_admin_router()` + 18 handler tests
- [x] **Step 4** — `ando-plugins`: 5 new plugins + 38 tests (50 total)
- [x] **Step 5** — `ando-observability`: 14 unit tests
- [x] **Step 6** — 10 pipeline integration tests (`ando-proxy/tests/integration.rs`)
- [x] **Step 7** — 3 proptest properties for Router
- [x] **Step 8** — CI workflow (`.github/workflows/ci.yml`) with 70% coverage gate
- [ ] **Step 9** — Measure actual line coverage; tune until all targets in Section 10 are met

**Next action:** run `cargo llvm-cov --workspace --html --open` and check which
paths fall below their targets.
