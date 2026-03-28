# Ando Community Edition (CE) — Todo

## Completed

### Infrastructure & Repo
- [x] Restructure CE repo layout (flatten v2/ to root, remove v1/ and benchmark/)
- [x] Remove dead code, clean clippy warnings
- [x] Fix EMFILE: raise ulimit at startup + lower default keepalive pool size
- [x] All repos pushed to GitHub remotes

### Tests — CE (407 passing, 0 failures)
- [x] ando-store unit tests (11)
- [x] ando-proxy unit tests (20 → 36)
- [x] ando-admin integration tests (18)
- [x] ando-plugins unit tests (64 → 81)
- [x] ando-observability tests (14 → 28)
- [x] ando-proxy pipeline integration tests (10)
- [x] ando-core proptest property tests (3)

### CI / Quality
- [x] GitHub Actions CI workflow (fmt + clippy + test + llvm-cov)
- [x] Coverage gate (≥70% lines) in CI

### Compliance
- [x] SOC2 / ISO 27001 / HIPAA / GDPR compliance controls wired in config
- [x] AuditLog + PII scrubber (33 tests)
- [x] SecurityHeaders plugin — HSTS, CSP, X-Frame-Options, etc. (17 tests)
- [x] COMPLIANCE.md — 20-row control matrix + quick-start profiles

### Proxy Features
- [x] Strip-prefix path rewriting (`strip_prefix` on Route)
- [x] Service layer (Route → Service → Upstream)
- [x] CORS middleware on admin router

### Dashboard (ando-ce/dashboard)
- [x] Replaced embedded HTML with standalone Next.js app (static export)
- [x] CRUD pages: Routes, Services, Upstreams, Consumers
- [x] EE locked plugins cards + upgrade modal
- [x] Test Request button — live proxy test with response panel
- [x] Descriptions, hints, and example placeholders in all dialogs

### Website (ando-website)
- [x] /docs, /community, /enterprise pages
- [x] Plugin gallery — CE active (green) + EE locked (upgrade overlay)
- [x] Security & Trust section on landing page

---

## Remaining — Roadmap

### Phase 1 — Core Parity
- [ ] **Health checks** — active (HTTP probe interval) + passive (5xx window) per upstream node; remove unhealthy nodes from rotation automatically
- [ ] **Request/response rewrite plugin** — add/remove/overwrite request headers, rewrite URI, set response headers
- [ ] **Limit-conn plugin** — max concurrent connections per route/consumer; queue or reject at cap
- [ ] **URI redirect plugin** — 301/302 redirect with path templates and regex capture groups
- [ ] **Consistent-hash load balancer** — sticky routing by client IP, header, or cookie value

### Phase 2 — AI Gateway (Community)
- [ ] **Unified AI Providers** — OpenAI-compatible interface for Anthropic, Gemini, Mistral, and local Ollama
- [ ] **Semantic Cache** — Vector-similarity caching (Redis VL or pgvector) for redundant LLM queries
- [ ] **Token-aware Rate Limiting** — Enforce TPM (Tokens Per Minute) and RPM limits per Consumer

### Phase 3 — OpenClaw Ecosystem Integration
- [ ] **OpenClaw Gateway RPC** — Native support for the OpenClaw RPC protocol to connect remote skills and tools
- [ ] **Agent Session Sticky Routing** — Ensure requests are routed to the same isolated agent workspace/session based on Agent-ID
- [ ] **Skill Execution Proxy** — Secure OpenClaw skill calls using Ando's Auth, PII Scrubber, and Rate Limiting layers
- [ ] **ClawHub Registry Sync** — Dynamically configure gateway routes by syncing with the ClawHub skill registry
- [ ] **A2UI Live Canvas Streaming** — Optimize Ando's proxy layer for the low-latency websocket requirements of Live Canvas

---

## Recently Completed
- [x] CE: file-based audit log rotation — daily + size-based with pruning (8 tests)
- [x] CE: e2e TCP integration tests already existed (9 tests in connection_integration.rs)
- [x] Dashboard: service picker in Routes modal already existed
- [x] CI: coverage gate already enforced at 85% lines (exceeds 70% target)
