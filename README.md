# Ando — Enterprise API Gateway

This repository contains two separate versions of the Ando API Gateway:

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
