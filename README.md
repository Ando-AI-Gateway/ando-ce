# Ando — High-Performance API Gateway

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

Ando is an open-source, high-performance API gateway built in Rust with a **monoio thread-per-core** architecture. It delivers near-native proxy performance by eliminating async overhead and cross-core contention.

## Performance

| Metric | Ando | APISIX | Kong |
|--------|------|--------|------|
| Plain Proxy (200c) | **288,960 req/s** | 155,108 | 125,803 |
| Key-Auth (200c) | **259,377 req/s** | 136,409 | 104,635 |
| Stress (500c) | **285,186 req/s** | 126,601 | 120,237 |
| **p99 Latency** | **2.64ms** | 4.33ms | 6.69ms |

**2x the throughput of APISIX, sub-3ms p99 latency.**

## Architecture

```
┌─────────────────────────────────────────────────┐
│                    Clients                       │
└────────────────────┬────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────┐
│              Ando Gateway                        │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │  Data Plane (monoio thread-per-core)      │   │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐    │   │
│  │  │Worker 0 │ │Worker 1 │ │Worker N │    │   │
│  │  │ Router  │ │ Router  │ │ Router  │    │   │
│  │  │ Plugins │ │ Plugins │ │ Plugins │    │   │
│  │  │ConnPool │ │ConnPool │ │ConnPool │    │   │
│  │  └─────────┘ └─────────┘ └─────────┘    │   │
│  └──────────────────────────────────────────┘   │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │  Control Plane (axum/tokio)               │   │
│  │  Admin API · Config Management            │   │
│  └──────────────────────────────────────────┘   │
└──────────────────────────────────────────────────┘
```

- **Thread-per-core**: Each CPU core runs a dedicated OS thread with its own monoio async runtime
- **Shared-nothing**: Workers operate on thread-local state — no locks, no DashMap on hot path
- **Zero-copy**: httparse zero-copy header parsing, pooled keepalive connections
- **Immutable router**: Radix trie built once, swapped atomically via `ArcSwap` (single atomic load per request)
- **jemalloc**: Optimized memory allocation with background threads

## Quick Start

### From Source
```bash
cargo build --release
./target/release/ando-server --config config/ando.yaml
```

### Docker
```bash
docker build -t ando:latest .
docker run -p 9080:9080 -p 9180:9180 ando:latest
```

### Docker Compose
```bash
docker compose up -d
```

## Configuration

See [config/ando.yaml](config/ando.yaml):

```yaml
proxy:
  http_addr: "0.0.0.0:9080"
  workers: 0              # 0 = auto-detect (one per CPU core)

admin:
  addr: "0.0.0.0:9180"
  enabled: true

deployment:
  mode: standalone        # standalone | etcd

observability:
  prometheus:
    enabled: false
    path: "/metrics"
```

All settings can be overridden via `ANDO_*` environment variables:
```bash
ANDO_PROXY_WORKERS=4 ANDO_ADMIN_ENABLED=true ./ando-server
```

## Admin API

APISIX-compatible REST API for dynamic configuration:

```bash
# Create a route
curl -X PUT http://localhost:9180/apisix/admin/routes/1 \
  -H "Content-Type: application/json" \
  -d '{
    "uri": "/api/*",
    "upstream": {
      "type": "roundrobin",
      "nodes": {"backend:8080": 1}
    }
  }'

# Add route with authentication
curl -X PUT http://localhost:9180/apisix/admin/routes/2 \
  -H "Content-Type: application/json" \
  -d '{
    "uri": "/secure/*",
    "upstream": {
      "type": "roundrobin",
      "nodes": {"backend:8080": 1}
    },
    "plugins": {
      "key-auth": {},
      "rate-limiting": {"rate": 100}
    }
  }'

# Create a consumer with API key
curl -X PUT http://localhost:9180/apisix/admin/consumers/myapp \
  -H "Content-Type: application/json" \
  -d '{
    "username": "myapp",
    "plugins": {
      "key-auth": {"key": "my-secret-key"}
    }
  }'
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `PUT` | `/apisix/admin/routes/{id}` | Create/update route |
| `GET` | `/apisix/admin/routes/{id}` | Get route |
| `DELETE` | `/apisix/admin/routes/{id}` | Delete route |
| `GET` | `/apisix/admin/routes` | List routes |
| `PUT` | `/apisix/admin/upstreams/{id}` | Create/update upstream |
| `GET` | `/apisix/admin/upstreams` | List upstreams |
| `PUT` | `/apisix/admin/consumers/{username}` | Create/update consumer |
| `GET` | `/apisix/admin/consumers` | List consumers |
| `GET` | `/apisix/admin/plugins/list` | List plugins |
| `GET` | `/apisix/admin/health` | Health check |

## Plugins

### Community Edition (included)

| Plugin | Type | Description |
|--------|------|-------------|
| `key-auth` | Authentication | API key authentication via header |
| `jwt-auth` | Authentication | JWT token validation (Bearer) |
| `basic-auth` | Authentication | HTTP Basic authentication |
| `ip-restriction` | Traffic Control | IP allow/deny lists (CIDR) |
| `rate-limiting` | Traffic Control | Local in-memory rate limiting |
| `cors` | Transform | Cross-Origin Resource Sharing |

### Enterprise Edition

For advanced features, see [Ando Enterprise](https://andogate.dev/enterprise):
- `hmac-auth` — HMAC request signing
- `oauth2` — OAuth 2.0 authentication
- `rate-limiting-advanced` — Distributed rate limiting (Redis)
- `traffic-mirror` — Request mirroring
- `canary-release` — Canary deployments
- `circuit-breaker` — Circuit breaker pattern
- etcd clustering & config hot-reload
- VictoriaMetrics/VictoriaLogs push
- OpenTelemetry distributed tracing
- Admin API authentication & RBAC
- Dashboard UI
- Multi-node deployment

## Crates

| Crate | Purpose |
|-------|---------|
| `ando-core` | Core types, config (Figment YAML+env), radix-trie router |
| `ando-proxy` | monoio thread-per-core data plane, connection pooling |
| `ando-plugin` | Plugin trait system, pipeline, registry |
| `ando-plugins` | Built-in CE plugins |
| `ando-store` | In-memory config cache (DashMap), optional etcd backend |
| `ando-observability` | Prometheus metrics, access logs |
| `ando-admin` | APISIX-compatible admin REST API (axum) |
| `ando-server` | Main binary, startup orchestration |

## Building with Enterprise Features

etcd and VictoriaMetrics support are available via Cargo features:

```bash
# CE default (standalone, prometheus scrape)
cargo build --release

# With etcd clustering
cargo build --release --features ando-store/etcd

# With VictoriaMetrics/Logs push
cargo build --release --features ando-observability/victoria

# All features
cargo build --release --features ando-store/etcd,ando-observability/victoria
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the [Apache License 2.0](LICENSE).

Copyright 2026 Ando Gateway Authors.
