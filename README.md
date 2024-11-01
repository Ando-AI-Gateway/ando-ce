# Ando — Enterprise API Gateway

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/kowito/ando/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)

**Ando** is a high-performance, cloud-native enterprise API gateway built on [Cloudflare Pingora](https://github.com/cloudflare/pingora). It provides feature parity with Apache APISIX while delivering superior performance through Rust's zero-cost abstractions, with a Lua Plugin Development Kit (PDK) powered by LuaJIT for extensibility.

## ✨ Key Features

- **Blazing Fast**: Built on Pingora's async, multithreaded architecture.
- **Apache APISIX Parity**: Compatible Admin API and Route/Service/Upstream models.
- **Lua PDK**: Write custom plugins in Lua without sacrificing performance.
- **Dynamic Config**: Real-time configuration updates via etcd.
- **Observability**: Built-in support for VictoriaMetrics (metrics) and VictoriaLogs (logging).
- **Edge Ready**: Minimal footprint, single static binary, and standalone mode support.

## 🚀 Quick Start

Get Ando running in under 2 minutes:

```bash
# Start the full stack (Ando + etcd + VictoriaMetrics/Logs)
cd deploy/docker
docker compose up -d

# Verify
curl http://localhost:9180/apisix/admin/health
```

For more detailed instructions, see the [**Quickstart Guide**](./QUICKSTART.md).

## 🏗️ Architecture

Ando consists of several core components:

- **Data Plane (ando-proxy)**: The request handling engine based on Pingora.
- **Control Plane (ando-admin)**: REST API for managing routes, upstreams, and plugins.
- **Plugin System (ando-plugin)**: Extensible pipeline supporting Rust and Lua plugins.
- **Storage (ando-store)**: etcd-backed configuration store with local caching.

Read more in [**ARCHITECTURE.md**](./ARCHITECTURE.md).

## 🧩 Plugins

Ando comes with built-in plugins for:
- **Authentication**: Key-auth, JWT, Basic-auth
- **Traffic Control**: Rate limiting (count, req)
- **Transformation**: Request/Response transformer, CORS
- **Security**: IP restriction
- **Observability**: Metrics and logging exporters

## 📜 License

This project is licensed under the [Apache-2.0 License](./LICENSE).
