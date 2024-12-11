# Ando V2 - Zero-Overhead Proxy Engine

A high-performance API gateway and reverse proxy built with **monoio** thread-per-core architecture, achieving near-native performance by eliminating async overhead and shared-state contention.

## Key Architecture

### Thread-Per-Core Model
- Each CPU core runs a dedicated OS thread with its own **monoio** async runtime
- **Shared-nothing data plane**: workers operate on thread-local state (HashMap caches, no locks)
- Control plane: Admin API runs on separate tokio thread for management
- Single shared resource: immutable frozen `Router` swapped atomically via `Arc<ArcSwap<Router>>`

### Hot Path Performance
- **Router**: matchit radix trie, built once and frozen—zero allocations on match
- **Plugin Pipeline**: Synchronous trait-based pipeline, no async overhead for simple plugins (key-auth)
- **Connection Handler**: HTTP/1.1 with httparse zero-copy parsing, keepalive pooling
- **Upstream Proxy**: Direct TcpStream without buffering (only reads data on demand)

### Why V2 Exists
**V1 (Pingora/Tokio)** bottlenecks on macOS Docker:
- Tokio task scheduler: cross-core work-stealing every context switch
- Shared DashMap for route cache: cache-line contention at scale
- nginx C event loop ceiling: ~5-10% of native performance on VM

**V2 solution:**
- Eliminate cross-core coordination → eliminate scheduler overhead
- Thread-local state → no contention, no DashMap atomics
- Result: 2-5x throughput on same hardware (depending on config)

## Building

### Prerequisites
- Rust 1.88+ (for `time` crate)
- macOS or Linux with io_uring/kqueue support

### From Source
```bash
cd v2
cargo build --release
```

### Docker
```bash
docker build -t ando-v2:latest .
```

## Running

### Basic Startup
```bash
cd v2
./target/release/ando-server --config config/ando.yaml
```

### Docker Compose (with benchmark)
```bash
cd ../benchmark
docker-compose up ando-v2
```

### Configuration
See [config/ando.yaml](config/ando.yaml) for defaults:
```yaml
proxy:
  address: "0.0.0.0:8000"
  workers: 4  # default: num_cpus

admin:
  address: "0.0.0.0:9000"
  enabled: true

etcd:
  enabled: false
  endpoints: ["http://127.0.0.1:2379"]

observability:
  access_logs: false
  metrics: false
  prometheus_enabled: false
```

**Key tuning:**
- `workers`: Set to # of performance cores (0 = auto-detect num_cpus)
- `etcd.enabled`: For dynamic config hot-reload via config watcher
- Observability: Disable (adds near-zero overhead) or enable for metrics export

## Crates Overview

| Crate | Purpose |
|-------|---------|
| **ando-core** | Frozen router (matchit), config/error types, upstream pool |
| **ando-proxy** | monoio workers, HTTP/1.1 handler, connection pool, proxy logic |
| **ando-plugin** | Plugin trait, registry, pipeline, execution phase model |
| **ando-plugins** | Built-in plugins: key-auth, traffic control |
| **ando-store** | ConfigCache (DashMap), etcd connector, config watcher |
| **ando-observability** | Prometheus metrics, access logs (no-op when disabled) |
| **ando-admin** | axum router on tokio thread, APISIX-compatible admin API |
| **ando-server** | Binary: startup, worker spawn, signal handling, jemalloc |

## Admin API

Both **v1 (Pingora)** and **v2 (monoio)** implement APISIX-compatible admin APIs.

### Endpoints
```bash
# Routes
PUT   /apisix/admin/routes/{id}      # Create/update
GET   /apisix/admin/routes/{id}      # Get
DELETE /apisix/admin/routes/{id}     # Delete
GET   /apisix/admin/routes           # List

# Upstreams
PUT   /apisix/admin/upstreams/{id}   # Create/update
GET   /apisix/admin/upstreams        # List

# Consumers (for auth plugins)
PUT   /apisix/admin/consumers/{id}    # Create/update
GET   /apisix/admin/consumers        # List

# Plugins
PUT   /apisix/admin/plugins/{id}     # Create/update
GET   /apisix/admin/plugins          # List
```

### Example: Add Route with Key-Auth
```bash
# 1. Add consumer with API key
curl -X PUT http://localhost:9000/apisix/admin/consumers/myapp \
  -H "Content-Type: application/json" \
  -d '{
    "username": "myapp",
    "plugins": {
      "key-auth": {
        "key": "secret-key-123"
      }
    }
  }'

# 2. Add route with key-auth plugin
curl -X PUT http://localhost:9000/apisix/admin/routes/r1 \
  -H "Content-Type: application/json" \
  -d '{
    "uri": "/api/*",
    "upstream": {
      "nodes": {"backend.example.com:80": 1},
      "type": "roundrobin"
    },
    "plugins": {
      "key-auth": {}
    }
  }'

# 3. Test
curl http://localhost:8000/api/test \
  -H "apikey: secret-key-123"  # Returns 401 without key
```

## Benchmarking

Compare v2, v1, and APISIX side-by-side:
```bash
cd ../benchmark
./bench.sh
```

Generates markdown report with:
- **Baseline**: Raw echo backend (no proxy)
- **Plain**: Proxy without plugins (v1, v2, APISIX)
- **Auth**: Key-auth plugin verification (v1, v2, APISIX)
- **Stress**: High-concurrency test (500 connections)
- **Ramp**: Throughput sweep 10→1000 concurrent connections

Results: `benchmark/results/latest_results.md` with Mermaid charts and 3-way comparison.

## Development

### Add Custom Plugin
1. Create in `ando-plugins/src/{your_plugin}/`:
   ```rust
   use ando_plugin::{Plugin, PluginInstance, Phase, PluginContext};

   pub struct MyPlugin;

   impl Plugin for MyPlugin {
       fn name(&self) -> &str { "my-plugin" }
       fn phases(&self) -> Vec<Phase> { vec![Phase::Access] }
       fn create_instance(&self, _cfg: &serde_json::Value) -> Result<Box<dyn PluginInstance>> {
           Ok(Box::new(MyPluginInstance))
       }
   }

   pub struct MyPluginInstance;
   impl PluginInstance for MyPluginInstance {
       fn execute(&self, ctx: &mut PluginContext) -> Result<i32> {
           // Your logic here (return 0 to continue)
           Ok(0)
       }
   }
   ```

2. Register in `ando-plugins/src/lib.rs`:
   ```rust
   pub fn register_plugins(registry: &mut PluginRegistry) {
       registry.register(Arc::new(MyPlugin));
   }
   ```

3. Use in config:
   ```yaml
   plugins:
     my-plugin: {}
   ```

### Compile & Test
```bash
cargo check          # Fast check
cargo build --release  # Full build
cargo test           # Run all tests
```

## Performance Notes

### Expected Throughput (macOS Docker / 4 cores)
- **Baseline (echo only)**: ~20k req/s (hardware ceiling)
- **v1 (Pingora/Tokio)**: ~3-5k req/s
- **v2 (monoio/thread-per-core)**: ~8-12k req/s (2-3x v1)
- **Native nginx**: ~18-20k req/s

### Why the gap?
- Docker macOS: nested VM + socket I/O overhead
- Linux native: v2 reaches ~15-18k req/s (monoio io_uring efficient)

### Tuning
- **Workers**: Match # physical cores (not threads)
- **etcd disabled**: ~5% faster (no config watch overhead)
- **Observability disabled**: Near-zero impact (no-op when off)
- **jemalloc**: Pre-configured in binary (better than libc malloc)

## Troubleshooting

### High CPU usage
- Check `workers` config ≤ physical cores
- Verify no runaway plugin logic (Access phase should be <100µs)

### Routes not updating
- Enable `etcd.enabled: true` in config
- Point to running etcd instance
- Check admin API reachability (default: port 9000)

### Poor performance
- Disable observability if not needed
- Check for sync I/O blocking in plugins
- Profile with `flamegraph` or `perf`

## See Also
- [V1 Documentation](../v1/README.md)
- [Main Architecture](../ARCHITECTURE.md)
- [Benchmark Results](../benchmark/)
