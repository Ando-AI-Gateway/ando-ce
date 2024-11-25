# Ando Benchmark Suite

Compares **Ando** (Rust/Pingora) against **APISIX** across real-world gateway scenarios — plain proxy, key-auth plugin, and high-concurrency stress.
Everything runs in Docker — no local toolchain required.

---

## Quick Start

```bash
# Requires: Docker Desktop (running)
./benchmark/bench.sh
```

`bench.sh` will:
1. **Build** Ando + echo-backend Docker images (`docker compose --build`)
2. **Start** `echo` (upstream), `etcd` (APISIX config store), `ando`, `apisix`
3. **Configure** Ando routes and APISIX routes via their Admin APIs
4. **Run** load tests (baseline → plain → auth → stress → ramp)
5. **Write** a Markdown report to `benchmark/results/`
6. **Stop** all containers on exit

---

## Architecture

```
  ┌──────────────────────────────────────┐
  │          Load Generator              │
  │   wrk  (200c, Nc threads, Xs)        │
  └────────┬─────────────────┬───────────┘
           │                 │
           ▼                 ▼
  ┌─────────────────┐  ┌─────────────────┐
  │  Ando  :9080    │  │ APISIX  :8080   │
  │  (Rust/Pingora) │  │ (Nginx/LuaJIT)  │
  │  Admin  :9180   │  │  Admin  :9181   │
  └────────┬────────┘  └────────┬────────┘
           │                    │
           └────────┬───────────┘
                    ▼
  ┌────────────────────────────────────┐
  │   Echo Backend  :3000              │
  │   (Rust / Hyper, keep-alive)       │
  └────────────────────────────────────┘
```

APISIX uses **etcd** as its configuration store (automatically started).

---

## Benchmark Scenarios

| # | Scenario | Connections | Duration | Description |
|---|---|---|---|---|
| 0 | **Baseline** | `BENCH_CONNECTIONS` | 30s | Direct hit to echo backend — establishes raw ceiling |
| 1 | **Plain Proxy** | `BENCH_CONNECTIONS` | `BENCH_DURATION` | Both gateways with no plugins — raw proxy throughput |
| 2 | **Key-Auth Plugin** | `BENCH_CONNECTIONS` | `BENCH_DURATION` | `key-auth` overhead compared head-to-head |
| 3 | **Stress Test** | `BENCH_STRESS_CONNECTIONS` | `BENCH_DURATION` | Push to saturation |
| 4 | **Ramp** | 10 → 1000 | 15s/step | Find saturation point |

---

## Running Specific Scenarios

```bash
./benchmark/bench.sh plain     # baseline + plain proxy
./benchmark/bench.sh auth      # key-auth plugin (both gateways)
./benchmark/bench.sh stress    # high-concurrency stress
./benchmark/bench.sh ramp      # concurrency ramp table
./benchmark/bench.sh baseline  # echo-backend ceiling only
./benchmark/bench.sh all       # everything (default)
```

### Override parameters

```bash
BENCH_DURATION=60s \
BENCH_CONNECTIONS=400 \
BENCH_STRESS_CONNECTIONS=1000 \
BENCH_THREADS=8 \
  ./benchmark/bench.sh all
```

---

## Prerequisites

| Tool | Required? |
|---|---|
| **Docker Desktop** (running) | Yes |

That's it — wrk, Rust, and all other tools run inside containers.

---

## Ports

| Service | Port | Description |
|---|---|---|
| Ando proxy | `9080` | HTTP under test |
| Ando admin | `9180` | Admin API |
| APISIX proxy | `8080` | HTTP under test |
| APISIX admin | `9180` | Admin API (key: `bench-admin-key-00000000000000`) |
| etcd | `2379` | APISIX config store |
| Echo backend | `3000` | Shared upstream |

---

## Results

Reports are written to `benchmark/results/report_YYYYMMDD_HHMMSS.md`.

```bash
# Open the latest report
open $(ls -t benchmark/results/*.md | head -1)

# List all reports
ls -lh benchmark/results/
```

Raw per-scenario wrk output is saved alongside as `wrk_<scenario>_<timestamp>.txt`.

---

## What's Measured

| Metric | Description |
|---|---|
| **RPS** | Requests per second |
| **p50 / p95 / p99** | Latency percentiles |
| **Proxy efficiency** | RPS relative to backend ceiling (100% = zero overhead) |
| **Saturation point** | Where RPS plateaus as connections increase |
| **Plugin overhead** | Delta between plain and key-auth RPS |

---

## Notes

- **First run is slow** — Ando compiles from Rust source (~2–3 min). Subsequent runs reuse the Docker layer cache.
- **APISIX startup**: APISIX waits for etcd to be healthy before starting; initial startup takes ~20s.
- **macOS ulimits**: For stress tests with 500+ connections, raise open-file limits first:
  ```bash
  ulimit -n 65536
  ```
- **CPU affinity**: For the most accurate results, run on a Linux machine with isolated CPU cores.

