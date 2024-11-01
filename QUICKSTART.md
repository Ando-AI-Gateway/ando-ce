# Ando Quickstart Guide

This guide will help you get Ando up and running in minutes.

## Prerequisites

- **Rust**: Latest stable version (1.80+)
- **Docker & Docker Compose**: For running the storage and observability stack
- **curl**: For testing the API

---

## 🚀 Option 1: The Quick Way (Docker Compose)

The easiest way to experience the full Ando stack (Ando + etcd + VictoriaMetrics + VictoriaLogs) is using Docker Compose.

1. **Clone and Enter the Directory**
   ```bash
   git clone https://github.com/kowito/ando.git
   cd ando
   ```

2. **Start the Stack**
   ```bash
   cd deploy/docker
   docker compose up -d
   ```

3. **Verify Installation**
   Check if those services are running:
   ```bash
   # Admin API Health Check
   curl http://localhost:9180/apisix/admin/health
   # Expected: {"status":"OK"}
   ```

---

## 🛠️ Option 2: Local Development

If you want to run Ando locally for development:

1. **Start etcd** (Ando needs etcd for configuration storage by default)
   ```bash
   docker run -d --name etcd -p 2379:2379 bitnami/etcd:3.5
   ```

2. **Build Ando**
   ```bash
   cargo build
   ```

3. **Run the Server**
   ```bash
   cargo run -p ando-server -- --config config/ando.yaml
   ```

---

## 📡 Your First Route

Let's configure Ando to proxy requests to `httpbin.org`.

### 1. Create an Upstream
The Upstream defines where the traffic should be sent.

```bash
curl -X POST http://localhost:9180/apisix/admin/upstreams \
-H "Content-Type: application/json" \
-d '{
  "id": "httpbin-upstream",
  "name": "Httpbin Backend",
  "type": "roundrobin",
  "nodes": {
    "httpbin.org:80": 1
  }
}'
```

### 2. Create a Route
The Route defines which incoming requests should match and which upstream to use.

```bash
curl -X POST http://localhost:9180/apisix/admin/routes \
-H "Content-Type: application/json" \
-d '{
  "id": "httpbin-get-route",
  "uri": "/get",
  "upstream_id": "httpbin-upstream"
}'
```

### 3. Test it!
Send a request to Ando's proxy port (`9080`).

```bash
curl http://localhost:9080/get
```
You should see a response from `httpbin.org` via Ando!

---

## 🧩 Adding a Plugin

Let's add a simple limit-count plugin to our route.

```bash
curl -X PUT http://localhost:9180/apisix/admin/routes/httpbin-get-route \
-H "Content-Type: application/json" \
-d '{
  "uri": "/get",
  "upstream_id": "httpbin-upstream",
  "plugins": {
    "limit-count": {
      "count": 2,
      "time_window": 60,
      "rejected_code": 429
    }
  }
}'
```

Test it by calling `curl http://localhost:9080/get` three times quickly. The third time should return a `429 Too Many Requests`.

---

## 🔍 Next Steps

- **Architecture**: Read [ARCHITECTURE.md](./ARCHITECTURE.md) to understand how Ando works.
- **Lua PDK**: Check out the `lua/examples` directory to see how to write custom plugins.
- **Admin API**: Ando provides an APISIX-compatible Admin API. You can use standard APISIX tools or dashboards.
