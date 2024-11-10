# Ando vs APISIX Benchmark Suite

A **heavy-concurrency** benchmark comparing [Ando](../) (Rust/Pingora) against [Apache APISIX](https://apisix.apache.org) across real-world gateway scenarios.

---

## Quick Start

```bash
# 1. Install a load tool
brew install wrk          # fast, latency percentiles
brew install wrk2         # most accurate (HDR histogram, constant-rate)
brew install k6           # scenario-based, spike/soak support
brew install hey          # simple fallback

# 2. Make scripts executable (first time only)
chmod +x benchmark/bench.sh benchmark/scripts/*.sh

# 3. Run the full benchmark
./benchmark/bench.sh
```

Results are saved to `benchmark/results/report_<timestamp>.md`.

---

## Architecture

```
  ┌─────────────────────────────────────────────┐
  │            Load Generator                   │
  │   wrk2 (500c, 12t, 50k rps target)         │
  │   k6   (2000 rps constant-rate, 600 VUs)   │
  └────────┬────────────────────────┬───────────┘
           │                        │
           ▼                        ▼
  ┌────────────────┐      ┌────────────────────┐
  │   Ando  :9080  │      │  APISIX  :8080     │
  │  (Rust/Pingora)│      │  (OpenResty/LuaJIT)│
  │  Admin : 9181  │      │  Admin   : 9180    │
  └────────┬───────┘      └────────┬───────────┘
           │                        │
           └───────────┬────────────┘
                       ▼
           ┌───────────────────────┐
           │   Echo Backend :3000  │
           │  (shared upstream)    │
           └───────────────────────┘
                etcd :2379 (shared, separate prefixes)
```

Both gateways share the same etcd instance (different key prefixes) and forward to the same echo backend — ensuring a fair apples-to-apples comparison.

---

## Benchmark Scenarios

### wrk / wrk2 Scenarios (`run_benchmark.sh`)

| # | Scenario | Connections | Duration | Description |
|---|---|---|---|---|
| 1 | **Plain Proxy** | 500 | 60s | Raw throughput — no plugins |
| 2 | **Key-Auth Plugin** | 500 | 60s | Plugin overhead with `key-auth` |
| 3 | **Concurrency Ramp** | 10→1000 | 20s/step | Find saturation point |
| 4 | **Stress Test** | 1000 | 60s | Push to limits — errors expected |

Default parameters (override with env vars):

```bash
BENCH_DURATION=60s            # duration per scenario
BENCH_CONNECTIONS=500         # concurrent connections (Scenarios 1, 2)
BENCH_THREADS=<2×CPU>         # wrk thread count (auto-detected)
BENCH_STRESS_CONNECTIONS=1000 # connections for stress scenario
BENCH_WRK2_RATE=50000         # target rps for wrk2 (HDR histogram mode)
```

### k6 Scenarios (`k6/benchmark.js`)

| Scenario | VUs | Target RPS | Duration | Purpose |
|---|---|---|---|---|
| `quick` | 50–200 | 500 | 15s | Smoke test |
| `heavy` | 200–600 | 2 000 (plain) / 1 000 (auth) | 60s each | Sustained load |
| `stress` | 500–1 500 | 500 → 15 000 (ramp) | 150s | Find breaking point |
| `spike` | 500–1 500 | 200 → 10 000 (burst) | 75s | Resilience to traffic spikes |
| `soak` | 200–600 | 1 000 | 10 min | Memory leaks, stability |

---

## Running Specific Scenarios

### wrk / wrk2

```bash
# Individual scenarios
./benchmark/bench.sh plain    # plain proxy only
./benchmark/bench.sh auth     # key-auth plugin only
./benchmark/bench.sh stress   # high-concurrency stress
./benchmark/bench.sh ramp     # concurrency scaling table

# Custom parameters
BENCH_DURATION=120s \
BENCH_CONNECTIONS=1000 \
BENCH_STRESS_CONNECTIONS=2000 \
./benchmark/bench.sh all
```

### k6

```bash
# Against Ando (local Docker stack)
k6 run --env TARGET=ando --env SCENARIO=heavy benchmark/k6/benchmark.js

# Against APISIX
k6 run --env TARGET=apisix --env SCENARIO=heavy benchmark/k6/benchmark.js

# Stress scenario
k6 run --env TARGET=ando --env SCENARIO=stress benchmark/k6/benchmark.js

# Spike resilience
k6 run --env TARGET=ando --env SCENARIO=spike benchmark/k6/benchmark.js

# Soak test (10 min)
k6 run --env TARGET=ando --env SCENARIO=soak benchmark/k6/benchmark.js
```

---

## What's Measured

| Metric | Description |
|---|---|
| **RPS** | Requests per second at target load |
| **p50 / p95 / p99** | Latency percentiles (median, tail, extreme tail) |
| **Error rate** | % of non-2xx responses |
| **Saturation point** | Concurrency level where RPS stops growing |
| **Plugin overhead** | Delta between plain and key-auth RPS/latency |

> **Why wrk2 over wrk?**  
> `wrk` suffers from [coordinated omission](https://www.youtube.com/watch?v=lJ8ydIuPFeU) — it stops timing during backpressure, making high-load latency look better than it is. `wrk2` uses HDR Histogram at a constant arrival rate, giving true latency percentiles.

---

## thresholds (k6 pass/fail)

```
latency_plain p95 < 100ms   ✓
latency_plain p99 < 250ms   ✓
latency_auth  p95 < 150ms   ✓
latency_auth  p99 < 300ms   ✓
error rate         < 1%     ✓
```

---

## Stack Management

```bash
# Start stack only
docker compose -f benchmark/docker-compose.bench.yml up -d

# Rebuild Ando after code change
docker compose -f benchmark/docker-compose.bench.yml up -d --build ando

# Tail logs
docker compose -f benchmark/docker-compose.bench.yml logs -f ando
docker compose -f benchmark/docker-compose.bench.yml logs -f apisix

# Stop & clean volumes
docker compose -f benchmark/docker-compose.bench.yml down -v
```

---

## Port Reference

| Service | Host Port | Description |
|---|---|---|
| **Ando** proxy | `9080` | HTTP under test |
| **Ando** admin | `9181` | Admin API |
| **APISIX** proxy | `8080` | HTTP under test |
| **APISIX** admin | `9180` | Admin API |
| Echo backend | `3000` | Shared upstream |
| etcd | `2379` | Config store |

---

## Results

Reports are saved as `benchmark/results/report_YYYYMMDD_HHMMSS.md`.

```bash
# Open the latest report
open $(ls -t benchmark/results/*.md | head -1)

# List all reports
ls -lh benchmark/results/
```

Raw per-run wrk output is saved alongside as `wrk_<scenario>_<timestamp>.txt`.

---

## Notes

- **First run is slow** — Ando is compiled from Rust source (~2–3 min). Subsequent runs use Docker layer cache.
- **Mac vs Linux**: Docker Desktop on Mac adds networking overhead (~10–30% lower RPS vs native Linux). For production-accurate results, run on a Linux VM or bare-metal server.
- **CPU pinning**: For maximal accuracy, isolate load generator and gateway onto separate CPU cores with `docker compose` CPU limits or run on separate machines.
- **OS tuning**: On Linux, increase `ulimit -n` (open file descriptors) and `net.core.somaxconn` kernel parameter before running 1000+ connection stress tests.
