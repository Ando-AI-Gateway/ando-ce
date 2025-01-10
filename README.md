# Ando — Enterprise API Gateway

Ando is a high-performance, cloud-native API gateway built in Rust. This repository contains two separate versions optimizing for different use cases: a mature production-ready v1 based on Pingora, and an experimental v2 with extreme performance using thread-per-core scheduling.

## Project Overview

**Ando v2** is currently the fastest API gateway in open-source benchmarks:

- **288,960 req/s** (plain proxy at 200 concurrent connections) — **1.9× APISIX**, 2.3× Kong
- **259,377 req/s** with key-based authentication 
- **2.64ms p99 latency** under load (vs 4.33ms APISIX, 6.69ms Kong)
- **285,186 req/s** under stress (500 connections) — outperforming all competitors

**Ando v1** offers production-proven reliability with ecosystem plugins:

- Built on Cloudflare's Pingora framework
- Full Lua/Wasm plugin support
- Dashboard UI with Next.js
- Mature routing, load balancing, and observability

Both versions expose an APISIX-compatible admin API for easy migration and integration.

## Latest Benchmark Results (Feb 21, 2026)

**Test Environment:** Apple M4 | **Duration:** 30s per scenario | **Load:** 4 worker threads

| Metric | Ando v2 | APISIX | Kong | Ando v1 | KrakenD | Tyk |
|--------|---------|--------|------|---------|---------|-----|
| Plain (200c) | 288,960 | 155,108 | 125,803 | 118,605 | 59,090 | 6,044 |
| Key-Auth (200c) | 259,377 | 136,409 | 104,635 | 113,534 | 61,343 | 5,451 |
| Stress (500c) | 285,186 | 126,601 | 120,237 | 133,805 | 50,738 | 5,338 |
| **p99 Latency** | **2.64ms** | 4.33ms | 6.69ms | 5.43ms | 13.06ms | 1,350ms |

**Key Findings:**
- Ando v2 **2× faster than APISIX** on throughput
- Sub-3ms p99 latency at 200 concurrent connections
- Maintains performance under stress (500c), where competitors degrade
- Full results with charts: [benchmark/results/20260221_140447/report.md](./benchmark/results/20260221_140447/report.md)

## Structure

```
ando/
├── benchmark/    # Unified benchmark: Ando v1 vs Ando v2 vs APISIX
│   ├── bench.sh
│   ├── docker-compose.yml
│   ├── ando-v1-bench.yaml
│   ├── ando-v2-bench.yaml
│   ├── apisix-config.yaml
│   ├── Dockerfile.echo
│   ├── Dockerfile.wrk
│   └── results/
│
├── v1/           # Ando v1 — Pingora/Tokio based
│   ├── Cargo.toml
│   ├── ando-core/
│   ├── ando-proxy/
│   ├── ando-plugin/
│   ├── ando-plugins/
│   ├── ando-store/
│   ├── ando-observability/
│   ├── ando-admin/
│   ├── ando-server/
│   └── ando-ui/
│
├── v2/           # Ando v2 — monoio thread-per-core, zero-overhead
│   ├── Cargo.toml
│   ├── ando-core/
│   ├── ando-proxy/
│   ├── ando-plugin/
│   ├── ando-plugins/
│   ├── ando-store/
│   ├── ando-observability/
│   ├── ando-admin/
│   └── ando-server/
│
└── README.md     # This file
```

## v1 — Pingora/Tokio

Built on [Cloudflare Pingora](https://github.com/cloudflare/pingora) + Tokio async runtime.

- Mature, production-tested proxy framework
- Full plugin ecosystem with Lua/Wasm support
- Dashboard UI (Next.js)

```bash
cd v1
cargo build --release
```

## v2 — monoio Thread-per-Core

Built on [ByteDance monoio](https://github.com/bytedance/monoio) — io_uring on Linux, kqueue on macOS.

**Architecture:**
- Thread-per-core, shared-nothing data plane
- Zero cross-core contention (no DashMap, no atomics on hot path)
- Frozen immutable router swapped via `ArcSwap` (single atomic load per request)
- Synchronous plugin pipeline (no async overhead for simple plugins)
- Admin API on separate tokio thread (axum)
- jemalloc allocator

**Target:** Beat APISIX at raw proxy throughput.

```bash
cd v2
cargo build --release
./target/release/ando-server -c config/ando.yaml
```

## Benchmark

A unified benchmark runs all three gateways side-by-side in Docker:

```bash
# Full benchmark (baseline + plain proxy + key-auth + stress + ramp)
./benchmark/bench.sh

# Single scenario
./benchmark/bench.sh plain

# Override params
BENCH_DURATION=60s BENCH_CONNECTIONS=400 ./benchmark/bench.sh all
```

Results are written to `benchmark/results/<timestamp>/report.md` with Mermaid charts for throughput and p99 latency.



Both versions expose an APISIX-compatible admin API at `/apisix/admin/*`:

```bash
# Create a route
curl -X PUT http://localhost:9180/apisix/admin/routes/1 \
  -H "Content-Type: application/json" \
  -d '{
    "uri": "/api/*",
    "methods": ["GET", "POST"],
    "upstream": {
      "type": "roundrobin",
      "nodes": {"backend:8080": 1}
    },
    "plugins": {
      "key-auth": {}
    }
  }'
```

## License

Apache-2.0
