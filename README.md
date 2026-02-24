# Ando CE — Zero-Overhead API Gateway

Ando Community Edition is a high-performance, cloud-native API gateway built in Rust on a monoio thread-per-core architecture.

## Performance

**Ando CE** is currently the fastest API gateway in open-source benchmarks:

- **288,960 req/s** (plain proxy at 200 concurrent connections) — **1.9× APISIX**, 2.3× Kong
- **259,377 req/s** with key-based authentication
- **2.64ms p99 latency** under load (vs 4.33ms APISIX, 6.69ms Kong)
- **285,186 req/s** under stress (500 connections) — outperforming all competitors

## Latest Benchmark Results (Feb 21, 2026)

**Test Environment:** Apple M4 | **Duration:** 30s per scenario | **Load:** 4 worker threads

| Metric | Ando CE | APISIX | Kong | KrakenD | Tyk |
|--------|---------|--------|------|---------|-----|
| Plain (200c) | 288,960 | 155,108 | 125,803 | 59,090 | 6,044 |
| Key-Auth (200c) | 259,377 | 136,409 | 104,635 | 61,343 | 5,451 |
| Stress (500c) | 285,186 | 126,601 | 120,237 | 50,738 | 5,338 |
| **p99 Latency** | **2.64ms** | 4.33ms | 6.69ms | 13.06ms | 1,350ms |

**Key Findings:**
- Ando CE **2× faster than APISIX** on throughput
- Sub-3ms p99 latency at 200 concurrent connections
- Maintains performance under stress (500c), where competitors degrade

## Architecture

Built on [ByteDance monoio](https://github.com/bytedance/monoio) — io_uring on Linux, kqueue on macOS.

- **Thread-per-core, shared-nothing data plane**: workers operate on thread-local state (no locks)
- **Frozen immutable router** swapped atomically via `ArcSwap` (single atomic load per request)
- **Synchronous plugin pipeline**: no async overhead for simple plugins
- **Admin API** on a separate tokio thread (axum)
- **jemalloc** allocator

## Structure

```
ando-ce/
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── config/
├── ando-core/
├── ando-proxy/
├── ando-plugin/
├── ando-plugins/
├── ando-store/
├── ando-observability/
├── ando-admin/
├── ando-server/
└── benchmark/    # Benchmark: Ando CE vs APISIX vs KrakenD vs Kong vs Tyk
    ├── bench.sh
    ├── docker-compose.yml
    ├── ando-ce-bench.yaml
    └── results/
```

## Quick Start

### Build & Run

```bash
# Build from source
cargo build --release
./target/release/ando-server -c config/ando.yaml

# Server listens on:
# - Proxy: http://localhost:9080
# - Admin API: http://localhost:8001
# - Dashboard: http://localhost:8001/admin/dashboard
```

### Docker

```bash
docker build -t ando-ce:latest .
docker run -p 9080:9080 -p 8001:8001 ando-ce:latest

# Or with docker-compose
docker-compose up -d
```

### First Route

Access the **Admin Dashboard** at `http://localhost:8001/admin/dashboard` and:

1. Click **+ Create Route**
2. Enter:
   - **Route ID**: `demo-route`
   - **URI**: `/demo/*`
   - **Upstream**: `http://httpbin.org` (or your backend)
3. Click **Save**

Test it:
```bash
curl http://localhost:9080/demo/get
```

## Admin API & Dashboard

### Web Dashboard

Modern admin UI at `http://localhost:8001/admin/dashboard`:

- **Routes** — Create, read, update, delete routes with live editor
- **Upstreams** — Manage backends (load balancing: round robin, chash, ewma)
- **Consumers** — Add API consumers with key-based authentication
- **Plugins** — View enabled plugins and their configuration
- **Settings** — Gateway info, connection details, edition
- **Test Request** — Send live requests through the proxy and inspect responses

![Dashboard features: CRUD routes/upstreams/consumers, live plugin visibility, test request panel]

### REST API

APISIX-compatible admin API at `/apisix/admin/*`:

```bash
# Create a route
curl -X PUT http://localhost:8001/apisix/admin/routes/demo \
  -H "Content-Type: application/json" \
  -d '{
    "uri": "/api/*",
    "methods": ["GET", "POST"],
    "upstream": {
      "type": "roundrobin",
      "nodes": {"backend.example.com:8080": 1}
    },
    "plugins": {
      "key-auth": {}
    }
  }'

# Get all routes
curl http://localhost:8001/apisix/admin/routes

# Delete a route
curl -X DELETE http://localhost:8001/apisix/admin/routes/demo
```

## Benchmark

```bash
# Full benchmark (CE vs APISIX vs KrakenD vs Kong vs Tyk)
./benchmark/bench.sh

# Single scenario
./benchmark/bench.sh plain

# Override params
BENCH_DURATION=60s BENCH_CONNECTIONS=400 ./benchmark/bench.sh all
```

Results are written to `benchmark/results/<timestamp>/report.md` with Mermaid charts.

## License

Apache-2.0
