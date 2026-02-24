# Testing Guide — Ando CE

This document is the authoritative reference for the test suite. All implementation
steps have been completed. **407 tests pass, 0 failures** across the workspace
(as of the last `cargo test --workspace` run).

**Line coverage: 88.79%** — above the 85% CI gate (infrastructure files excluded, see CI section).

---

## Current State

| Crate | Unit tests | Integration | Status |
|---|---|---|---|
| `ando-core` (error, route, router, upstream, consumer, config) | 59 (incl. 3 proptest) | — | ✅ done |
| `ando-plugin` (plugin, pipeline, registry) | 21 | — | ✅ done |
| `ando-plugins` (key-auth, basic-auth, jwt-auth, ip-restriction, rate-limiting, cors, security-headers) | 81 | — | ✅ done |
| `ando-proxy` | 36 | 10 (pipeline) + 2 (monoio E2E) | ✅ done |
| `ando-admin` | 18 (handler integration via `tower::ServiceExt`) | — | ✅ done |
| `ando-store` (cache, schema) | 80 | — | ✅ done |
| `ando-observability` (access_log, audit_log, metrics, logger, prometheus, pii_scrubber) | 57 | — | ✅ done |

**Total: 407 tests** — run with:

```bash
cargo test --workspace
```

---

## 1. Dev-dependencies in place

| Crate | Dev-dep added |
|---|---|
| `ando-core` | `proptest = "1"`, `tempfile = "3"` |
| `ando-proxy` | `ando-plugins` (path dep, for plugin pipeline tests) |
| `ando-admin` | `tower = { version = "0.4", features = ["util"] }` |
| workspace | `base64 = "0.22"` |

Workspace already includes `tokio`, `reqwest`, `arc-swap`, `jsonwebtoken`, `ipnet`, `base64`.

---

## 2. `ando-store` — ConfigCache + Schema ✅

### [`cache.rs`](ando-store/src/cache.rs) — 11 tests

| Test group | Tests |
|---|---|
| `find_consumer_by_key` | found after rebuild, unknown key, not-yet-rebuilt |
| `rebuild_consumer_key_index` | stale key replaced, multiple consumers, consumer without plugin |
| `all_routes` | empty, after insert, after remove, clone shares DashMap |
| `Default` | all maps empty |

### [`schema.rs`](ando-store/src/schema.rs) — **18 tests** (100% line coverage)

| Test group | Tests |
|---|---|
| All `prefix_*` methods | routes, upstreams, consumers, plugins, meta (absolute paths) |
| All `key_*` methods | route(id), upstream(id), consumer(username), plugin(name), meta(key) |
| Constructor edge cases | trailing-slash stripped, custom prefix, absolute path inserted once |
| Key uniqueness | all namespace keys are distinct for same id |

> **Note:** The `ando-store` crate also includes the etcd backend (`etcd.rs`, `watcher.rs`) and
> file-backed store tests; the 80-test total includes all of these.

---

## 3. `ando-proxy` — ProxyWorker (unit) ✅

File: [`ando-proxy/src/proxy.rs`](ando-proxy/src/proxy.rs) — **36 tests**

| Test group | Tests |
|---|---|
| `status_text` | all known codes + unknown fallback |
| `build_response` | status line, body, custom headers, buffer clear |
| `build_upstream_request` | format, hop-by-hop header filter, content-length |
| `handle_request` — routing | 404, fast-path proxy, disabled route, wildcard, method gate |
| `handle_request` — key-auth | missing key (plugin 401), invalid key (static 401), valid key (proxy) |
| `maybe_update_router` | no-op on same version, swaps on new version |
| `upstream_addresses` | returns inline route nodes |
| `compute_upstream_path` | strip_prefix: no-op, strip prefix, trailing slash, wildcard, deep path |
| `handle_request` — strip_prefix | strip enabled proxies correct path, strip disabled keeps full path |

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

All 7 plugins implemented and tested — **81 tests total**.

| Plugin | File | Tests | What's covered |
|---|---|---|---|
| `key-auth` | [`auth/key_auth.rs`](ando-plugins/src/auth/key_auth.rs) | 12 | default/custom header, hide-credentials, valid/missing/empty key |
| `basic-auth` | [`auth/basic_auth.rs`](ando-plugins/src/auth/basic_auth.rs) | 13 | missing header, Bearer scheme, invalid b64, no colon, valid creds, colons-in-password, empty password, lowercase prefix, **configure() + trait methods** |
| `jwt-auth` | [`auth/jwt_auth.rs`](ando-plugins/src/auth/jwt_auth.rs) | 17 | missing header, valid token, expired, wrong secret, no-Bearer, malformed, **configure() HS256/384/512, unknown algo, missing secret, custom header, no-sub token** |
| `ip-restriction` | [`traffic/ip_restriction.rs`](ando-plugins/src/traffic/ip_restriction.rs) | 12 | no restrictions, denylist direct/CIDR, allowlist allow/block, denylist priority, **configure() empty/with-lists/invalid-CIDR, trait methods** |
| `rate-limiting` | [`traffic/rate_limiting.rs`](ando-plugins/src/traffic/rate_limiting.rs) | 10 | within limit, exceeds limit (429), independent IPs, window reset, zero limit, **configure() valid/missing-fields, instance enforcement, trait methods** |
| `cors` | [`traffic/cors.rs`](ando-plugins/src/traffic/cors.rs) | 11 | no origin header, wildcard, specific list allow/block, OPTIONS 204, CORS headers, allow-credentials, **configure() valid/invalid, trait methods** |
| `security-headers` | [`traffic/security_headers.rs`](ando-plugins/src/traffic/security_headers.rs) | 17 | all headers emitted (HSTS, X-Frame-Options, X-Content-Type-Options, X-XSS-Protection, Referrer-Policy, CSP, Permissions-Policy), cache-control no-store (HIPAA), configure() valid/invalid, plugin name/priority |

All plugins registered in [`lib.rs`](ando-plugins/src/lib.rs) `register_all()`.

**Notable edge case:** `ip_restriction::configure()` uses `filter_map` — invalid CIDRs are **silently ignored** (this is intentional, matching production behavior). The test `configure_with_invalid_cidr_silently_ignores_bad_entry` verifies the valid CIDR in the same list is still applied.

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

## 6a. `ando-proxy` — monoio E2E connection tests ✅

File: [`ando-proxy/tests/connection_integration.rs`](ando-proxy/tests/connection_integration.rs) — **2 tests**

Uses `monoio::RuntimeBuilder` (LegacyDriver / kqueue) with real TCP sockets.
Each test spins a monoio runtime inline, connects a real TCP stream to
`handle_connection()`, and checks the HTTP response status line.

| Test | Scenario | Expected |
|---|---|---|
| `handle_connection_404_no_matching_route` | Empty router, known path | 404 Not Found |
| `handle_connection_plugin_blocks_with_401` | key-auth plugin, no API key | 401 Unauthorized |

---

## 6b. `ando-core` — config tests ✅

Tests added inline to [`ando-core/src/config.rs`](ando-core/src/config.rs) — **15 + 4 compliance tests = 19 tests** (98.18%+ line coverage)

| Test group | Tests |
|---|---|
| `Default` impls | `WorkerConfig`, `ProxyConfig`, `TlsConfig`, `ObservabilityConfig`, `LoggingConfig`, `MetricsConfig`, `AdminConfig`, `EtcdConfig` |
| `effective_workers` | zero → `num_cpus`, explicit value returned as-is |
| `DeploymentMode` serde | `standalone`/`cluster` roundtrip via YAML |
| `GatewayConfig::load()` | nonexistent file → error, valid YAML → config, etcd mode parse, observability overrides |
| `ComplianceConfig` defaults | all flags disabled by default |
| `TlsComplianceConfig` defaults | `min_version = TLSv1.2`, `strict_ciphers = false` |
| `AuditLogConfig` + `PiiScrubConfig` roundtrip | YAML serialise/deserialise |
| `ComplianceConfig` YAML load | full compliance section parsed correctly |

New dev-dep: `tempfile = "3"` (for `NamedTempFile` in load tests).

---

## 7. `ando-observability` — unit tests ✅

**57 tests** across six files.

### [`access_log.rs`](ando-observability/src/access_log.rs) — 6 tests

| Test | What it verifies |
|---|---|
| `serialises_all_fields` | all `AccessLogEntry` fields present in JSON output |
| `upstream_addr_none_serialises_to_null` | `Option<String>` → JSON null |
| `roundtrip_with_upstream` | JSON serialize → deserialize preserves values |
| `roundtrip_without_upstream` | same, without upstream addr |
| `various_status_codes_serialise_correctly` | 200, 400, 404, 429, 500, 502, etc. |
| `debug_format_does_not_panic` | `{:?}` formatting |

### [`audit_log.rs`](ando-observability/src/audit_log.rs) — 11 tests

| Test | What it verifies |
|---|---|
| `new_sets_all_fields` | all `AuditLogEntry` fields initialized correctly |
| `to_json_line_is_valid_utf8` | output is newline-terminated JSON |
| `allow_outcome_serialises_correctly` | `AuditOutcome::Allow` → `"ALLOW"` |
| `deny_outcome_serialises_correctly` | `AuditOutcome::Deny` → `"DENY"` |
| `deny_helper_sets_deny_fields` | `.deny(plugin, reason)` sets outcome + plugin + reason |
| `consumer_id_is_none_by_default` | unauthenticated requests have no consumer |
| `roundtrip_serialisation` | JSON serialize → deserialize preserves all fields |
| `hipaa_required_fields_present` | request_body_hash, consumer_id non-None when set |
| `request_id_is_included_in_output` | request_id always present in SIEM output |
| `duration_ms_is_float` | latency serialised as float |
| `pii_scrubbed_flag` | pii_scrubbed field present and serialises correctly |

### [`pii_scrubber.rs`](ando-observability/src/pii_scrubber.rs) — 22 tests

| Test | What it verifies |
|---|---|
| `scrub_known_sensitive_headers` | Authorization, Cookie, Set-Cookie, X-Api-Key masked |
| `scrub_header_unknown_passthrough` | non-sensitive headers left unchanged |
| `scrub_headers_map_*` | bulk header scrubbing on `HashMap<String,String>` |
| `anonymize_ipv4_*` | last octet zeroed (GDPR Art. 32) |
| `anonymize_ipv6_*` | last 64 bits zeroed |
| `scrub_uri_*` | query-param value redaction with configurable regex patterns |
| `compile_patterns_*` | empty list, valid regex, invalid regex skipped |

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

### [`logger.rs`](ando-observability/src/logger.rs) — 6 tests (previously 0%)

| Test | What it verifies |
|---|---|
| `disabled_constructor_has_no_sender` | `disabled()` → `sender` is `None` |
| `new_with_disabled_config_has_no_sender` | `new(disabled_config)` → `sender` is `None` |
| `access_log_noop_when_disabled` | `access_log()` on disabled logger doesn't panic |
| `enabled_logger_has_sender` | `new(enabled_config)` → `sender` is `Some` |
| `enabled_logger_access_log_does_not_block` | `access_log()` returns immediately |
| `backpressure_does_not_panic` | filling channel buffer doesn't panic |

### [`prometheus_exporter.rs`](ando-observability/src/prometheus_exporter.rs) — 4 tests (previously 0%, now 100%)

| Test | What it verifies |
|---|---|
| `empty_registry_returns_empty_string` | empty metrics → empty output |
| `counter_appears_in_output` | registered counter shows in Prometheus text format |
| `gauge_appears_in_output` | registered gauge shows in Prometheus text format |
| `output_is_valid_utf8` | output is always valid UTF-8 |

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
1. `cargo llvm-cov --workspace --lcov --ignore-filename-regex '...'` (via `taiki-e/install-action`)
2. Codecov upload (non-fatal)
3. `cargo llvm-cov report --fail-under-lines 85` — enforces **85% gate**

**Excluded files** (infrastructure-bound, not unit-testable):

| File | Reason |
|---|---|
| `main.rs` | Binary entry point; never executed in unit/integration tests |
| `etcd.rs` | Requires a live etcd cluster (integration env only) |
| `watcher.rs` | Requires a live etcd cluster (integration env only) |
| `ssl.rs` | Requires TLS certificate infrastructure |
| `worker.rs` | OS thread + socket bootstrap (`spawn_workers`/`worker_loop`); logic covered by integration |

Triggers: push to `main`, `ce/*`, `feat/*`; all pull requests.

---

## 10. Coverage summary (actual, as of production-readiness sprint)

**Overall: 88.79% line / 90.76% function** (infrastructure files excluded).

| File | Line coverage | Notes |
|---|---|---|
| `ando-core/src/config.rs` | **98.18%** | ✅ |
| `ando-core/src/router.rs` | 96.70% | ✅ |
| `ando-store/src/cache.rs` | 100.00% | ✅ |
| `ando-store/src/schema.rs` | 100.00% | ✅ |
| `ando-observability/src/prometheus_exporter.rs` | 100.00% | ✅ |
| `ando-plugins/*/jwt_auth.rs` | **93.56%** | ✅ |
| `ando-plugins/*/rate_limiting.rs` | 94.66% | ✅ |
| `ando-plugins/*/cors.rs` | 95.32% | ✅ |
| `ando-plugins/*/ip_restriction.rs` | 90.74% | ✅ |
| `ando-observability/src/logger.rs` | 76.09% | ⚠️ flush_loop HTTP task (needs live VictoriaLogs) |
| `ando-proxy/src/connection.rs` | 42.04% | ⚠️ streaming/keepalive paths (needs traffic replay) |
| `ando-proxy/src/worker.rs` | 17.57% | ⚠️ excluded from gate — OS bootstrap code |

Run locally:

```bash
cargo install cargo-llvm-cov --locked
cargo llvm-cov --workspace \
  --ignore-filename-regex '(main\.rs|etcd\.rs|watcher\.rs|ssl\.rs|worker\.rs)' \
  --html --open
```

---

## 11. Implementation checklist

- [x] **Step 1** — `ando-store`: 11 ConfigCache unit tests
- [x] **Step 2** — `ando-proxy`: 20 ProxyWorker unit tests (+ 16 strip_prefix tests later)
- [x] **Step 3** — `ando-admin`: `build_admin_router()` + 18 handler tests
- [x] **Step 4** — `ando-plugins`: 5 new plugins + 38 tests (50 total)
- [x] **Step 5** — `ando-observability`: 14 unit tests
- [x] **Step 6** — 10 pipeline integration tests (`ando-proxy/tests/integration.rs`)
- [x] **Step 7** — 3 proptest properties for Router
- [x] **Step 8** — CI workflow (`.github/workflows/ci.yml`) with coverage gate
- [x] **Step 9** — Production-readiness coverage sprint
  - `schema.rs`: 0% → 100% (18 new tests)
  - `prometheus_exporter.rs`: 0% → 100% (4 new tests)
  - `logger.rs`: 0% → 76% (6 new tests)
  - `config.rs`: 79% → 98% (15 new tests, added `tempfile` dev-dep)
  - `jwt_auth.rs`: 52% fn → 88% fn (10 new configure() tests)
  - Plugin trait tests: basic-auth, cors, ip-restriction, rate-limiting (14 new tests)
  - `ando-proxy/tests/connection_integration.rs`: 2 monoio E2E TCP tests
  - CI: `worker.rs` added to exclusions; gate raised 70% → 85%
- [x] **Step 10** — Compliance sprint: SOC2 / ISO 27001 / HIPAA / GDPR controls
  - `ando-core/src/config.rs`: `ComplianceConfig`, `TlsComplianceConfig`, `AuditLogConfig`, `PiiScrubConfig` (4 new tests)
  - `ando-observability/src/audit_log.rs` (new): 11 tests
  - `ando-observability/src/pii_scrubber.rs` (new): 22 tests
  - `ando-plugins/src/traffic/security_headers.rs` (new): 17 tests
  - `COMPLIANCE.md`: 20-control matrix + quick-start profiles + shared responsibility model
- [x] **Step 11** — feature: `strip_prefix` path rewriting — 7 new proxy unit tests
- [x] **Step 12** — Dead code removal + docs update
  - Removed `config: Arc<GatewayConfig>` dead field from `ProxyWorker`
  - Removed empty `middleware` module from `ando-admin`
  - Removed stale "v2 design:" comment from `ando-admin/src/server.rs`
  - Updated README: correct structure tree, fixed port typo, removed stale benchmark section
  - Updated TESTING.md: 261 → 407 tests, added security-headers plugin, audit_log, pii_scrubber

**Known infrastructure-bound gaps** (not covered by unit tests — require infra):

| File | Coverage | What's needed to improve |
|---|---|---|
| `worker.rs` | 17% (excluded) | Full server start with real SO_REUSEPORT sockets |
| `connection.rs` | 42% | Multi-chunk streaming, keepalive, stale connection retry |
| `ando-admin/src/server.rs` | 62% | `serve_admin()` startup (needs real TCP bind) |
| `logger.rs` flush_loop | (untestable) | Live VictoriaLogs HTTP endpoint |
| `etcd.rs`, `watcher.rs` | 0% (excluded) | Live etcd cluster — use `testcontainers` crate |
