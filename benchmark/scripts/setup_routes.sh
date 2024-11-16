#!/usr/bin/env bash
# ============================================================
# setup_routes.sh — Bootstrap identical routes on Ando & APISIX
# ============================================================
# Creates a "benchmark" route on both gateways pointing to the
# shared echo backend, with and without key-auth plugin.
#
# Usage:
#   ./scripts/setup_routes.sh
# ============================================================

set -euo pipefail

ANDO_ADMIN="${ANDO_ADMIN:-http://localhost:9181}"
APISIX_ADMIN="${APISIX_ADMIN:-http://localhost:9180}"
APISIX_KEY="${APISIX_KEY:-edd1c9f034335f136f87ad84b625c8f1}"
ECHO_UPSTREAM="${ECHO_UPSTREAM:-http://echo:80}"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()    { echo -e "${BLUE}[INFO]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC}  $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC} $*"; }

wait_for() {
  local name="$1" url="$2" header="${3:-}" tries=30
  info "Waiting for $name to be ready..."
  for i in $(seq 1 $tries); do
    if [ -n "$header" ]; then
      code=$(curl -s -o /dev/null -w "%{http_code}" -H "$header" "$url" 2>/dev/null || echo "000")
    else
      code=$(curl -s -o /dev/null -w "%{http_code}" "$url" 2>/dev/null || echo "000")
    fi
    if [[ "$code" =~ ^[23] ]]; then
      success "$name is ready"
      return 0
    fi
    sleep 2
  done
  warn "$name did not become ready in time (last code: $code)."
}

# ── Wait for gateways ─────────────────────────────────────────
wait_for "Ando Admin" \
  "${ANDO_ADMIN}/apisix/admin/health"

wait_for "APISIX Admin" \
  "${APISIX_ADMIN}/apisix/admin/routes" \
  "X-API-KEY: ${APISIX_KEY}"

# ============================================================
# ANDO ROUTES
# ============================================================
info "Setting up Ando routes..."

# --- Upstream ---
curl -s -X PUT "${ANDO_ADMIN}/apisix/admin/upstreams/bench-echo" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "bench-echo",
    "type": "roundrobin",
    "nodes": {
      "echo:80": 1
    }
  }' | jq -c '{id: .id, status: "created"}' 2>/dev/null || true
success "Ando upstream: bench-echo"

# --- Route: plain proxy (no plugin) ---
curl -s -X PUT "${ANDO_ADMIN}/apisix/admin/routes/bench-plain" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "bench-plain",
    "uri": "/bench/plain",
    "methods": ["GET", "POST"],
    "upstream_id": "bench-echo",
    "plugins": {}
  }' | jq -c '{id: .id, status: "created"}' 2>/dev/null || true
success "Ando route: /bench/plain (no plugins)"

# --- Route: key-auth ---
curl -s -X PUT "${ANDO_ADMIN}/apisix/admin/consumers/bench-user" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "bench-user",
    "plugins": {
      "key-auth": {
        "key": "bench-secret-key"
      }
    }
  }' | jq -c '{username: .username, status: "created"}' 2>/dev/null || true
success "Ando consumer: bench-user"

curl -s -X PUT "${ANDO_ADMIN}/apisix/admin/routes/bench-auth" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "bench-auth",
    "uri": "/bench/auth",
    "methods": ["GET", "POST"],
    "upstream_id": "bench-echo",
    "plugins": {
      "key-auth": {}
    }
  }' | jq -c '{id: .id, status: "created"}' 2>/dev/null || true
success "Ando route: /bench/auth (key-auth plugin)"

echo ""
# ============================================================
# APISIX ROUTES
# ============================================================
info "Setting up APISIX routes..."

# --- Upstream ---
curl -s -X PUT "${APISIX_ADMIN}/apisix/admin/upstreams/1" \
  -H "X-API-KEY: ${APISIX_KEY}" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 1,
    "type": "roundrobin",
    "nodes": {
      "echo:80": 1
    }
  }' | jq -c '{id: .value.id, status: "created"}' 2>/dev/null || true
success "APISIX upstream: echo:80"

# --- Route: plain proxy (no plugin) ---
curl -s -X PUT "${APISIX_ADMIN}/apisix/admin/routes/1" \
  -H "X-API-KEY: ${APISIX_KEY}" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 1,
    "uri": "/bench/plain",
    "methods": ["GET", "POST"],
    "upstream_id": 1,
    "plugins": {}
  }' | jq -c '{id: .value.id, status: "created"}' 2>/dev/null || true
success "APISIX route: /bench/plain (no plugins)"

# --- Consumer + key-auth ---
curl -s -X PUT "${APISIX_ADMIN}/apisix/admin/consumers/bench-user" \
  -H "X-API-KEY: ${APISIX_KEY}" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "bench-user",
    "plugins": {
      "key-auth": {
        "key": "bench-secret-key"
      }
    }
  }' | jq -c '{username: .value.username, status: "created"}' 2>/dev/null || true
success "APISIX consumer: bench-user"

curl -s -X PUT "${APISIX_ADMIN}/apisix/admin/routes/2" \
  -H "X-API-KEY: ${APISIX_KEY}" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 2,
    "uri": "/bench/auth",
    "methods": ["GET", "POST"],
    "upstream_id": 1,
    "plugins": {
      "key-auth": {}
    }
  }' | jq -c '{id: .value.id, status: "created"}' 2>/dev/null || true
success "APISIX route: /bench/auth (key-auth plugin)"

echo ""
success "All routes configured!"
echo ""
echo "  Test Ando:  curl http://localhost:9080/bench/plain"
echo "  Test APISIX: curl http://localhost:8080/bench/plain"
