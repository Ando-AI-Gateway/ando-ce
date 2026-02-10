# Ando Features Matrix

This document outlines the complete feature set across **Ando Community Edition (CE)** and **Ando Enterprise Edition (EE)**.

---

## Core Engine

Both editions run the same high-performance data plane built in Rust.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **Thread-per-core architecture** | ✓ | ✓ | Workers pinned to CPU cores. No cross-thread scheduling. |
| **monoio async runtime** | ✓ | ✓ | Thread-per-core event loop using io_uring (Linux) or kqueue (macOS). |
| **Shared-nothing workers** | ✓ | ✓ | Each worker owns its connections. Zero mutable state on hot path. |
| **ArcSwap router reload** | ✓ | ✓ | Atomic config swaps. One atomic load per request. |
| **288,960 req/s throughput** | ✓ | ✓ | Baseline performance on Apple M4. |
| **2.64ms p99 latency** | ✓ | ✓ | Deterministic latency under load. |

---

## Admin API (Control Plane)

Manage routes, upstreams, consumers, and services via REST.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **APISIX-compatible Admin API** | ✓ | ✓ | Port 9080 for proxy, port 9180 for admin. Compatible with APISIX tooling. |
| **Routes management** | ✓ | ✓ | Create, read, update, delete routes with URI patterns and upstreams. |
| **Upstreams management** | ✓ | ✓ | Define upstream servers with load balancing (roundrobin). |
| **Consumers management** | ✓ | ✓ | Create consumers for API key, JWT, Basic auth per route. |
| **Services management** | ✓ | ✓ | Bundle routes and plugins into logical services. |
| **SSL / TLS certificates** | ✓ | ✓ | Upload and manage HTTPS certificates per domain. |
| **Health check endpoint** | ✓ | ✓ | `/apisix/admin/health` returns gateway status. |
| **RBAC (Role-Based Access Control)** | ✗ | ✓ | Separate credentials per operator with scoped admin API access. |

---

## Standalone Mode

Run Ando without clustering — all config via Admin API.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **Standalone deployment** | ✓ | ✓ | Single node. Config stored in memory. Reload via Admin API. |
| **Admin API persistence** | ✓ | ✓ | In-memory DashMap for routes, upstreams, consumers. |
| **Configuration via YAML** | ✓ | ✓ | ando.yaml file for initial config (proxy ports, workers, timeouts). |
| **Environment variable overrides** | ✓ | ✓ | ANDO_* prefix for config values at runtime. |

---

## Cluster Mode (etcd-backed)

Multi-node deployment with shared config.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **etcd integration** | ✓ | ✓ | Store routes, upstreams, consumers in etcd. Watch for live updates. |
| **Config hot-reload** | ✓ | ✓ | Changes propagate to all nodes in < 5ms. No restart required. |
| **Watcher thread** | ✓ | ✓ | Dedicated tokio thread watches etcd for config changes. |
| **Multi-node HA** | ✗ | ✓ | Leader election. Automatic failover. State consistency. |
| **Leader election** | ✗ | ✓ | etcd-based leader selection. Handles node failures gracefully. |

---

## Authentication Plugins

Extract and validate credentials from incoming requests.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **key-auth (API Key)** | ✓ | ✓ | Validate API key in header against consumer store. |
| **jwt-auth** | ✓ | ✓ | JWT Bearer token extraction. HS256 and RS256 support. |
| **basic-auth** | ✓ | ✓ | HTTP Basic auth (username:password base64 encoded). |
| **hmac-auth** | ✗ | ✓ | HMAC request signing and verification. Replay protection. |
| **oauth2** | ✗ | ✓ | Full OAuth 2.0 flows: authorization code, client credentials, token introspection. |

---

## Traffic Control Plugins

Manage, shape, and direct traffic.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **ip-restriction** | ✓ | ✓ | Allow or deny requests by IP address or CIDR range. |
| **rate-limiting (local)** | ✓ | ✓ | Sliding-window rate limiter in-memory. Per-consumer or per-IP. |
| **rate-limiting-advanced (distributed)** | ✗ | ✓ | Redis-backed rate limiting. Consistent limits across cluster nodes. |
| **traffic-mirror** | ✗ | ✓ | Shadow production traffic to secondary upstream. Test without live impact. |
| **canary-release** | ✗ | ✓ | Weighted traffic splitting between stable and canary upstreams. Gradual rollouts. |
| **circuit-breaker** | ✗ | ✓ | Automatic upstream isolation on failure. Configurable thresholds and recovery. |

---

## Request/Response Transformation

Modify headers, body, and request flow.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **cors** | ✓ | ✓ | CORS headers with preflight OPTIONS handling. Configurable origins. |
| **header-filter phase** | ✓ | ✓ | Plugins can modify request headers before upstream. |
| **body-filter phase** | ✓ | ✓ | Plugins can modify response body after upstream. |
| **request rewriting** | ✓ | ✓ | Rewrite URI, method, headers in Rewrite phase. |

---

## Observability & Monitoring

Metrics, logs, and health visibility.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **Prometheus metrics** | ✓ | ✓ | Request counters, latency histograms, upstream health. Scrape from `/metrics`. |
| **Access logs** | ✓ | ✓ | Structured request/response logs (status, latency, consumer, route). |
| **VictoriaMetrics push** | ✓ | ✓ | Push metrics to VictoriaMetrics. Configurable batch size and interval. |
| **Request tracing** | ✓ | ✓ | Per-request context tracking (route_id, consumer, client_ip). |
| **Advanced dashboard** | ✗ | ✓ | Real-time traffic visualizations, upstream health, per-route metrics. |
| **Per-consumer analytics** | ✗ | ✓ | Track usage per consumer. Quota reporting and alerts. |

---

## Configuration & Deployment

How Ando is packaged and run.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **Docker image** | ✓ | ✓ | Official image: `ghcr.io/andolabs/ando:latest`. Multi-stage build. |
| **docker-compose** | ✓ | ✓ | Standalone and etcd cluster configurations. |
| **Bare metal** | ✓ | ✓ | Binary compilation and deployment on Linux/macOS. |
| **Kubernetes (Helm)** | ✗ | ✓ | Production-ready Helm chart. StatefulSet for multi-node clusters. |
| **Rolling updates** | ✓ | ✓ | Zero-downtime config reload. No request loss. |
| **Health checks** | ✓ | ✓ | Liveness and readiness probes for orchestrators. |
| **Resource limits** | ✓ | ✓ | Memory and CPU control via config. jemalloc allocator for efficiency. |

---

## Plugin System

Extend Ando with custom logic.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **Plugin trait** | ✓ | ✓ | Rust trait-based plugin interface. Synchronous by default. |
| **6-phase pipeline** | ✓ | ✓ | Rewrite → Access → BeforeProxy → HeaderFilter → BodyFilter → Log. |
| **Priority ordering** | ✓ | ✓ | Plugins sorted by priority within each phase. |
| **Performance** | ✓ | ✓ | Runs on monoio worker thread. No async overhead for simple plugins. |
| **Built-in plugins** | ✓ (6) | ✓ (12) | CE: 6 plugins. EE: CE + 6 additional plugins. |
| **Custom plugins** | ✗ | ✓ | Ando team develops and maintains custom plugins for your use case. |
| **Plugin marketplace** | ✗ | ✓ | Community plugins (with SLA support). |

---

## Support & Services

Help accessing your deployment.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **Documentation** | ✓ | ✓ | Quickstart, config reference, plugin docs, architecture guide. |
| **GitHub issues** | ✓ | ✓ | Community issue tracker. Contributions welcome. |
| **Community forum** | ✓ | ✓ | Discussions and Q&A (GitHub Discussions). |
| **Priority SLA support** | ✗ | ✓ | Guaranteed response times. Dedicated support team. |
| **Architecture review** | ✗ | ✓ | Ando team reviews your setup and suggests optimizations. |
| **Training** | ✗ | ✓ | On-site or remote training for your ops team. |
| **Professional services** | ✗ | ✓ | Custom development, migration planning, performance tuning. |

---

## Licensing & Commercial

Legal and commercial terms.

| Feature | CE | EE | Description |
|---------|----|----|-------------|
| **License** | Apache 2.0 | Proprietary | CE: Open source. EE: Proprietary with commercial support. |
| **Source code** | Public | Private | CE: GitHub public. EE: Private repository. |
| **Redistribution** | ✓ | ✗ | CE: Allowed under Apache 2.0. EE: Not allowed. |
| **Modification** | ✓ | ✗ | CE: Allowed. EE: Requires Ando team approval. |
| **Commercial use** | ✓ | ✓ | Both allowed at any scale. |
| **No license key required** | ✓ | ✗ | CE: Truly open. EE: Requires license key. |

---

## Quick Comparison

### For Teams Starting Out

**Community Edition** is ideal if you:
- Want to evaluate Ando's performance
- Run a single node or simple cluster
- Need local rate limiting (no distributed state)
- Don't require advanced traffic management features
- Prefer open-source infrastructure

### For Scale & Reliability

**Enterprise Edition** adds if you need:
- Multi-node HA clustering with automatic failover
- Distributed rate limiting (Redis-backed)
- Advanced traffic management (canary, mirror, circuit-breaker)
- RBAC admin interface for team access control
- Dedicated engineering support with SLA
- Advanced monitoring and analytics dashboard

---

## Feature by Use Case

### High-Traffic AI Inference Gateway
- **Data plane**: Both (identical performance)
- **Clustering**: EE (multi-node HA)
- **Rate limiting**: EE (distributed via Redis)
- **Traffic shaping**: EE (canary, mirror, circuit-breaker)
- **Monitoring**: EE (advanced dashboard, per-consumer analytics)

### Internal Microservices Gateway
- **Data plane**: Both (sufficient)
- **Clustering**: CE (etcd-backed, no HA needed)
- **Rate limiting**: CE (local sliding-window)
- **Auth**: Both (JWT, key-auth, basic-auth)
- **Monitoring**: Both (Prometheus metrics, access logs)

### API Management at Scale
- **Data plane**: Both (high throughput)
- **Clustering**: EE (multi-node, automatic failover)
- **Auth**: EE (OAuth2, HMAC, advanced JWT)
- **Traffic shaping**: EE (canary releases, circuit-breaker)
- **Admin**: EE (RBAC, team access control)
- **Support**: EE (SLA, dedicated team)

---

## Getting Started

- **Community Edition**: [github.com/andolabs/ando](https://github.com/andolabs/ando)
- **Enterprise Edition**: [Contact sales](mailto:enterprise@andolabs.org)

See the [Pricing](/pricing) page for licensing details.
