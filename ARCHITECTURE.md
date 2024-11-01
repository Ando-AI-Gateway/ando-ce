# Ando — Enterprise API Gateway

## Overview

**Ando** is a high-performance, cloud-native enterprise API gateway built on [Cloudflare Pingora](https://github.com/cloudflare/pingora). It provides feature parity with Apache APISIX while delivering superior performance through Rust's zero-cost abstractions, with a Lua Plugin Development Kit (PDK) powered by LuaJIT for extensibility.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Ando API Gateway                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │  Admin API   │  │  Control     │  │   Data Plane          │  │
│  │  (REST/gRPC) │  │  Plane       │  │   (Pingora Proxy)     │  │
│  │              │  │              │  │                       │  │
│  │  - Routes    │  │  - Config    │  │  ┌─────────────────┐  │  │
│  │  - Services  │  │    Sync      │  │  │  Router (Trie)  │  │  │
│  │  - Upstreams │  │  - Watch     │  │  │  + Radix Match  │  │  │
│  │  - Plugins   │  │    etcd      │  │  └────────┬────────┘  │  │
│  │  - SSL Certs │  │  - Health    │  │           │           │  │
│  │  - Consumers │  │    Check     │  │  ┌────────▼────────┐  │  │
│  └──────┬───────┘  └──────┬───────┘  │  │ Plugin Pipeline │  │  │
│         │                 │          │  │                 │  │  │
│         │                 │          │  │ ┌─────────────┐ │  │  │
│         │                 │          │  │ │ Rust Plugin  │ │  │  │
│         │                 │          │  │ │ (native)     │ │  │  │
│         │                 │          │  │ ├─────────────┤ │  │  │
│         │                 │          │  │ │ Lua Plugin   │ │  │  │
│         │                 │          │  │ │ (LuaJIT/PDK) │ │  │  │
│         │                 │          │  │ ├─────────────┤ │  │  │
│         │                 │          │  │ │ WASM Plugin  │ │  │  │
│         │                 │          │  │ │ (wasmtime)   │ │  │  │
│         │                 │          │  │ └─────────────┘ │  │  │
│         │                 │          │  └────────┬────────┘  │  │
│         │                 │          │           │           │  │
│         │                 │          │  ┌────────▼────────┐  │  │
│         │                 │          │  │  Load Balancer  │  │  │
│         │                 │          │  │  + Health Check │  │  │
│         │                 │          │  └────────┬────────┘  │  │
│         │                 │          │           │           │  │
│         │                 │          │  ┌────────▼────────┐  │  │
│         │                 │          │  │   Upstream      │  │  │
│         │                 │          │  │   Manager       │  │  │
│         │                 │          │  └─────────────────┘  │  │
│         │                 │          └───────────────────────┘  │
├─────────┴─────────────────┴────────────────────────────────────┤
│                     Storage & Observability                     │
│                                                                 │
│  ┌──────────────┐   ┌──────────────────┐  ┌──────────────────┐ │
│  │    etcd       │   │ VictoriaMetrics  │  │  VictoriaLogs   │ │
│  │              │   │                  │  │                  │ │
│  │  - Routes    │   │  - Request       │  │  - Access Logs   │ │
│  │  - Services  │   │    Metrics       │  │  - Error Logs    │ │
│  │  - Upstreams │   │  - Latency       │  │  - Plugin Logs   │ │
│  │  - Plugins   │   │  - Bandwidth     │  │  - Audit Trail   │ │
│  │  - SSL       │   │  - Health        │  │                  │ │
│  │  - Consumers │   │  - Plugin        │  │                  │ │
│  └──────────────┘   └──────────────────┘  └──────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Workspace Structure

```
ando/
├── Cargo.toml                    # Workspace root
├── ARCHITECTURE.md               # This document
│
├── ando-core/                    # Core types, config, router
│   ├── src/
│   │   ├── lib.rs
│   │   ├── config.rs             # Configuration (figment-based)
│   │   ├── error.rs              # Error types (thiserror)
│   │   ├── router.rs             # Radix-trie router (matchit)
│   │   ├── route.rs              # Route model (APISIX-compatible)
│   │   ├── service.rs            # Service model
│   │   ├── upstream.rs           # Upstream + LB + health check config
│   │   ├── consumer.rs           # Consumer model
│   │   ├── ssl.rs                # SSL certificate model
│   │   └── plugin_config.rs      # Reusable plugin config sets
│   └── Cargo.toml
│
├── ando-proxy/                   # Pingora-based proxy engine
│   ├── src/
│   │   ├── lib.rs
│   │   ├── proxy.rs              # ProxyHttp impl (request lifecycle)
│   │   ├── balancer.rs           # Weighted round-robin load balancer
│   │   └── health_check.rs       # Active health checking
│   └── Cargo.toml
│
├── ando-plugin/                  # Plugin system & Lua PDK
│   ├── src/
│   │   ├── lib.rs
│   │   ├── plugin.rs             # Plugin trait, PluginContext, PluginInstance
│   │   ├── pipeline.rs           # Phase-based plugin execution pipeline
│   │   ├── registry.rs           # Thread-safe plugin registry
│   │   └── lua/
│   │       ├── mod.rs
│   │       ├── runtime.rs        # Lua plugin loader & executor
│   │       ├── pdk.rs            # Lua PDK (ando.request/response/log/ctx/json)
│   │       └── pool.rs           # LuaJIT VM pool
│   └── Cargo.toml
│
├── ando-plugins/                 # Built-in plugins
│   ├── src/
│   │   ├── lib.rs                # Plugin registration
│   │   ├── auth/
│   │   │   ├── key_auth.rs       # API key authentication
│   │   │   ├── jwt_auth.rs       # JWT authentication (HS256/RS256)
│   │   │   └── basic_auth.rs     # HTTP Basic authentication
│   │   ├── traffic/
│   │   │   ├── limit_count.rs    # Fixed-window rate limiter
│   │   │   └── limit_req.rs      # Leaky bucket rate limiter (stub)
│   │   ├── transform/
│   │   │   ├── cors.rs           # CORS (preflight + response headers)
│   │   │   ├── request_transformer.rs   # Add/remove/rename request headers
│   │   │   └── response_transformer.rs  # Add/remove response headers
│   │   ├── security/
│   │   │   └── ip_restriction.rs # CIDR-based IP allow/deny lists
│   │   └── observability/
│   │       └── mod.rs            # Observability plugin stubs
│   └── Cargo.toml
│
├── ando-store/                   # etcd storage layer
│   ├── src/
│   │   ├── lib.rs
│   │   ├── etcd.rs               # etcd client wrapper (CRUD)
│   │   ├── watcher.rs            # Real-time config sync from etcd
│   │   ├── cache.rs              # DashMap-based in-memory cache
│   │   └── schema.rs             # etcd key schema (/ando/routes/...)
│   └── Cargo.toml
│
├── ando-observability/           # Metrics & logging
│   ├── src/
│   │   ├── lib.rs
│   │   ├── metrics.rs            # Prometheus metrics + VM push
│   │   ├── logger.rs             # VictoriaLogs NDJSON exporter
│   │   ├── access_log.rs         # Structured access log format
│   │   └── prometheus_exporter.rs # Text exposition helper
│   └── Cargo.toml
│
├── ando-admin/                   # Admin REST API (Axum)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── server.rs             # Axum server + APISIX-compatible routes
│   │   ├── middleware.rs          # API key auth middleware
│   │   └── handlers/
│   │       ├── mod.rs
│   │       ├── routes.rs         # CRUD for routes
│   │       ├── services.rs       # CRUD for services
│   │       ├── upstreams.rs      # CRUD for upstreams
│   │       ├── consumers.rs      # CRUD for consumers
│   │       ├── ssl.rs            # CRUD for SSL certificates
│   │       ├── plugins.rs        # Plugin listing
│   │       └── health.rs         # Health check + schema
│   └── Cargo.toml
│
├── ando-server/                  # Main binary
│   ├── src/
│   │   └── main.rs               # CLI, bootstrap, Pingora server setup
│   └── Cargo.toml
│
├── lua/                          # Lua PDK library & examples
│   ├── ando/
│   │   └── pdk.lua               # PDK entry point (re-exports ando.*)
│   └── examples/
│       ├── hello_world.lua       # Example: custom headers plugin
│       └── custom_auth.lua       # Example: token auth plugin
│
├── config/
│   └── ando.yaml                 # Default configuration file
│
└── deploy/
    ├── docker/
    │   ├── Dockerfile            # Multi-stage production build
    │   └── docker-compose.yml    # Full stack (Ando + etcd + VM + VL)
    └── edge/
        ├── Dockerfile.edge       # Minimal distroless edge image
        └── edge-config.yaml      # Edge-optimized config
```

## Key Design Decisions

### 1. Pingora as the Foundation
Pingora provides the async, multithreaded proxy framework with HTTP/1, HTTP/2, gRPC, and WebSocket support. We implement `ProxyHttp` trait to integrate our plugin pipeline into the request lifecycle.

### 2. Lua Plugin Performance
- **LuaJIT** via `mlua` crate with `luajit` + `vendored` features
- **VM Pool**: Pre-warmed pool of LuaJIT VMs to avoid creation overhead
- **Shared bytecode cache**: Plugins compiled once, reused across VMs
- **Async bridge**: Lua coroutines bridge to Tokio async runtime
- **Zero-copy where possible**: Use FFI for buffer access

### 3. Plugin Execution Phases (APISIX-compatible)
1. `rewrite` — Modify the request before routing
2. `access` — Authentication, authorization, rate limiting
3. `before_proxy` — Just before proxying upstream
4. `header_filter` — Modify response headers
5. `body_filter` — Modify response body
6. `log` — Post-response logging

### 4. Edge Deployment
- Single static binary (musl target)
- Minimal resource footprint
- Standalone mode (no etcd required, file-based config)
- Sub-10MB Docker image
- ARM64 + x86_64 cross-compilation

### 5. Storage
- **etcd** as primary config store with watch for real-time sync
- **Local cache** for zero-latency config reads
- **Standalone mode** with YAML/JSON file config (for edge)

### 6. Observability
- **VictoriaMetrics**: Push metrics via Prometheus remote write protocol
- **VictoriaLogs**: Push logs via JSON stream API (`/insert/jsonline`)
- **Prometheus endpoint**: `/metrics` for pull-based scraping
