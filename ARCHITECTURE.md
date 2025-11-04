# Ando — CE/EE Architecture

This document describes the architecture for maintaining Ando as an open-core product
with a Community Edition (CE) and Enterprise Edition (EE).

## Repository Layout

```
github.com/andogate/
├── ando                    # CE — Public (Apache 2.0)
│   ├── ando-core/          # Core types, config, router
│   ├── ando-proxy/         # monoio thread-per-core data plane
│   ├── ando-plugin/        # Plugin trait system
│   ├── ando-plugins/       # CE plugins (key-auth, jwt, basic, ip, rate-limit, cors)
│   ├── ando-store/         # ConfigCache + optional etcd (feature-gated)
│   ├── ando-observability/  # Prometheus scrape + optional Victoria push
│   ├── ando-admin/         # APISIX-compatible admin API
│   ├── ando-server/        # CE binary
│   ├── config/
│   ├── Cargo.toml
│   ├── LICENSE             # Apache 2.0
│   └── README.md
│
└── ando-enterprise         # EE — Private (Proprietary)
    ├── ando-plugins-ee/    # Enterprise plugins
    ├── ando-admin-ee/      # Admin auth, RBAC
    ├── ando-cluster/       # etcd clustering, leader election
    ├── ando-server-ee/     # EE binary (links CE + EE)
    ├── Cargo.toml          # Depends on CE via git
    ├── LICENSE             # Proprietary
    └── README.md
```

## How EE Depends on CE

EE's workspace `Cargo.toml` pulls CE crates as git dependencies:

```toml
[workspace.dependencies]
ando-core = { git = "https://github.com/andogate/ando.git", branch = "main" }
ando-proxy = { git = "https://github.com/andogate/ando.git", branch = "main" }
ando-plugin = { git = "https://github.com/andogate/ando.git", branch = "main" }
ando-plugins = { git = "https://github.com/andogate/ando.git", branch = "main" }
ando-store = { git = "...", features = ["etcd"] }
ando-observability = { git = "...", features = ["prometheus", "victoria"] }
ando-admin = { git = "https://github.com/andogate/ando.git", branch = "main" }
```

When CE is updated, EE pulls the latest with `cargo update`.

## Feature Gating Strategy

CE uses Cargo features to gate enterprise-ready code:

### ando-store
```toml
[features]
default = []                           # CE: in-memory only
etcd = ["dep:etcd-client", "dep:tokio", "dep:crossbeam-channel"]
```

### ando-observability
```toml
[features]
default = ["prometheus"]               # CE: prometheus scrape
prometheus = ["dep:prometheus"]
victoria = ["dep:reqwest", "dep:tokio", "dep:chrono"]
```

This means:
- `cargo build` → CE build (standalone, prometheus)
- `cargo build --features ando-store/etcd,ando-observability/victoria` → Full features
- EE's Cargo.toml enables all features automatically

## CE vs EE Feature Matrix

| Feature | CE | EE |
|---------|----|----|
| **Proxy Engine** | ✅ monoio thread-per-core | ✅ Same |
| **Router** | ✅ Radix trie + ArcSwap | ✅ Same |
| **Plugin System** | ✅ Full trait system | ✅ Same + EE plugins |
| **Admin API** | ✅ CRUD routes/upstreams/consumers | ✅ + Auth + RBAC |
| **Config** | ✅ YAML + env vars | ✅ + etcd clustering |
| | | |
| **Auth: key-auth** | ✅ | ✅ |
| **Auth: jwt-auth** | ✅ | ✅ |
| **Auth: basic-auth** | ✅ | ✅ |
| **Auth: hmac-auth** | ❌ | ✅ |
| **Auth: oauth2** | ❌ | ✅ |
| | | |
| **Traffic: ip-restriction** | ✅ | ✅ |
| **Traffic: rate-limiting** | ✅ (local/in-memory) | ✅ (local) |
| **Traffic: rate-limiting-advanced** | ❌ | ✅ (Redis distributed) |
| **Traffic: traffic-mirror** | ❌ | ✅ |
| **Traffic: canary-release** | ❌ | ✅ |
| **Traffic: circuit-breaker** | ❌ | ✅ |
| | | |
| **Transform: cors** | ✅ | ✅ |
| | | |
| **Observability: Prometheus scrape** | ✅ | ✅ |
| **Observability: VictoriaMetrics push** | ❌ | ✅ |
| **Observability: VictoriaLogs push** | ❌ | ✅ |
| **Observability: OpenTelemetry** | ❌ | ✅ |
| **Observability: Stdout logging** | ✅ | ✅ |
| | | |
| **Deployment: Standalone** | ✅ | ✅ |
| **Deployment: etcd clustering** | ❌ | ✅ |
| **Deployment: Multi-node** | ❌ | ✅ |
| **Deployment: Leader election** | ❌ | ✅ |
| | | |
| **Dashboard UI** | ❌ | ✅ (planned) |
| **SSL/TLS management** | ❌ | ✅ |
| **Active health checks** | ❌ | ✅ |
| **Audit logging** | ❌ | ✅ |

## Maintenance Workflow

### Day-to-day Development

1. **Most work happens in CE** — core proxy, basic plugins, admin API
2. **EE-only work** happens in the private repo
3. CE is the upstream truth — EE never modifies CE code
4. EE extends via: plugin registration, middleware layers, feature flags

### Release Process

1. Tag CE with version: `git tag v0.2.0`
2. Update EE's git dep to the tag: `ando-core = { git = "...", tag = "v0.2.0" }`
3. Test EE build with updated CE deps
4. Tag EE with matching version

### Adding a New Feature

**If it belongs in CE:**
1. Implement in the `ando` repo
2. Add tests, update README
3. EE gets it automatically on next `cargo update`

**If it belongs in EE:**
1. Check if CE needs new extension points (traits, hooks)
2. If yes: add the extension point in CE first (traits are free/open)
3. Implement the feature in `ando-enterprise`

**Adding a new plugin:**
1. Just implement `Plugin` + `PluginInstance` traits
2. Register in `register_all()` (CE) or `register_ee_plugins()` (EE)
3. No changes needed to the proxy engine

## Extension Points

CE provides these extension points that EE hooks into:

| Extension Point | CE Location | How EE Uses It |
|----------------|-------------|----------------|
| `PluginRegistry` | `ando-plugin/registry.rs` | EE calls `register_ee_plugins()` to add plugins |
| `Plugin` trait | `ando-plugin/plugin.rs` | EE implements new plugins |
| `ConfigCache` | `ando-store/cache.rs` | EE wraps with etcd persistence |
| `AdminState` | `ando-admin/server.rs` | EE wraps with auth middleware |
| Cargo features | `ando-store`, `ando-observability` | EE enables `etcd`, `victoria` features |
| Config struct | `ando-core/config.rs` | EE extends with `figment` layers |
