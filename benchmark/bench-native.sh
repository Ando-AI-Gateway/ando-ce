#!/usr/bin/env bash
# ============================================================
# bench-native.sh — Run Ando benchmark WITHOUT Docker
# ============================================================
# Starts everything locally:
#   1. Go echo backend on port 3000
#   2. Ando (release build) in standalone mode on port 9080
#   3. Creates benchmark routes via Admin API
#   4. Runs wrk load tests (plain + key-auth + stress)
#   5. Cleans up, writes report
#
# Prerequisites:
#   brew install wrk
#   go 1.20+ (for the echo backend)
#
# Usage:
#   ./benchmark/bench-native.sh [plain|auth|stress|all]
# ============================================================

set -euo pipefail
trap cleanup EXIT

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RESULTS_DIR="${SCRIPT_DIR}/results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
REPORT_FILE="${RESULTS_DIR}/native_report_${TIMESTAMP}.md"
SCENARIO="${1:-all}"

# ---- Ports ----
ANDO_PROXY_PORT=9080
ANDO_ADMIN_PORT=9180
ECHO_PORT=3000

# ---- Benchmark params ----
DURATION="${BENCH_DURATION:-30s}"
CONNECTIONS="${BENCH_CONNECTIONS:-200}"
THREADS="${BENCH_THREADS:-$(sysctl -n hw.logicalcpu 2>/dev/null || echo 8)}"
STRESS_CONNECTIONS="${BENCH_STRESS_CONNECTIONS:-500}"
API_KEY="bench-secret-key"

ANDO_PID=""
ECHO_PID=""

# ---- Colors ----
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

mkdir -p "${RESULTS_DIR}"

info()    { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()      { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()     { echo -e "${RED}[ERR]${NC}   $*"; }
header()  { echo -e "\n${BOLD}${CYAN}════════════════════════════════════════${NC}"; echo -e "${BOLD}  $*${NC}"; echo -e "${BOLD}${CYAN}════════════════════════════════════════${NC}"; }

# ---- Cleanup ----
cleanup() {
  echo ""
  info "Cleaning up processes..."
  [ -n "${ANDO_PID}" ] && kill "${ANDO_PID}" 2>/dev/null && info "Stopped Ando (PID ${ANDO_PID})"
  [ -n "${ECHO_PID}" ] && kill "${ECHO_PID}" 2>/dev/null && info "Stopped echo backend (PID ${ECHO_PID})"
  # Also clear any lingering listeners on our ports
  lsof -ti:${ANDO_PROXY_PORT},${ANDO_ADMIN_PORT},${ECHO_PORT} | xargs kill -9 2>/dev/null || true
}

# ---- Wait for port to open ----
wait_for_port() {
  local name="$1" port="$2" tries=40
  info "Waiting for ${name} on port ${port}..."
  for i in $(seq 1 $tries); do
    if nc -z 127.0.0.1 "${port}" 2>/dev/null; then
      ok "${name} is up on :${port}"
      return 0
    fi
    sleep 0.5
  done
  err "${name} did not come up on :${port} after ${tries} tries!"
  exit 1
}

# ---- Wait for Admin API health endpoint ----
wait_for_admin() {
  local tries=40
  info "Waiting for Ando Admin API..."
  for i in $(seq 1 $tries); do
    if curl -sf "http://127.0.0.1:${ANDO_ADMIN_PORT}/apisix/admin/health" > /dev/null 2>&1; then
      ok "Ando Admin is ready"
      return 0
    fi
    sleep 0.5
  done
  err "Ando Admin API did not respond after ${tries} tries!"
  exit 1
}

# ---- Start Go echo backend ----
start_backend() {
  header "Step 1: Starting echo backend on port ${ECHO_PORT}"
  local echo_dir="${SCRIPT_DIR}/echo-backend"
  go run "${echo_dir}/main.go" --port "${ECHO_PORT}" &
  ECHO_PID=$!
  wait_for_port "Echo backend" "${ECHO_PORT}"
}

# ---- Build and start Ando ----
start_ando() {
  header "Step 2: Building and starting Ando (Release mode)"
  info "Running cargo build --release..."
  cargo build --release --bin ando -q 2>&1
  ok "Ando built successfully"

  # Write a minimal standalone config
  local cfg_file="${RESULTS_DIR}/bench-native.yaml"
  cat > "${cfg_file}" <<YAML
proxy:
  http_addr: "0.0.0.0:${ANDO_PROXY_PORT}"
  https_addr: "0.0.0.0:9443"
  workers: 0
  connect_timeout_ms: 3000
  read_timeout_ms: 10000
  write_timeout_ms: 10000

admin:
  addr: "127.0.0.1:${ANDO_ADMIN_PORT}"
  enabled: true

deployment:
  mode: standalone

observability:
  victoria_metrics:
    enabled: false
  victoria_logs:
    enabled: false

lua:
  plugin_dir: "/tmp/ando-bench-plugins"
  pool_size: 16
  timeout_ms: 5000
  max_memory: 67108864
YAML

  mkdir -p /tmp/ando-bench-plugins

  "${PROJECT_ROOT}/target/release/ando" --config "${cfg_file}" \
    --log-level warn > "${RESULTS_DIR}/ando-native.log" 2>&1 &
  ANDO_PID=$!
  wait_for_admin
}

# ---- Create benchmark routes ----
setup_routes() {
  header "Step 3: Creating benchmark routes"
  local admin="http://127.0.0.1:${ANDO_ADMIN_PORT}/apisix/admin"

  # --- Plain proxy route ---
  curl -sf -X PUT "${admin}/routes/bench-plain" \
    -H "Content-Type: application/json" \
    -d "{
      \"id\": \"bench-plain\",
      \"uri\": \"/bench\",
      \"upstream\": {
        \"type\": \"roundrobin\",
        \"nodes\": { \"127.0.0.1:${ECHO_PORT}\": 1 }
      },
      \"plugins\": {}
    }" | jq -c '{id: .id}' 2>/dev/null || true
  ok "Route created: /bench (plain, no plugins)"

  # --- Consumer for key-auth ---
  curl -sf -X PUT "${admin}/consumers/bench-user" \
    -H "Content-Type: application/json" \
    -d "{
      \"username\": \"bench-user\",
      \"plugins\": {
        \"key-auth\": { \"key\": \"${API_KEY}\" }
      }
    }" | jq -c '{username: .username}' 2>/dev/null || true

  # --- Key-auth route ---
  curl -sf -X PUT "${admin}/routes/bench-auth" \
    -H "Content-Type: application/json" \
    -d "{
      \"id\": \"bench-auth\",
      \"uri\": \"/bench-auth\",
      \"upstream\": {
        \"type\": \"roundrobin\",
        \"nodes\": { \"127.0.0.1:${ECHO_PORT}\": 1 }
      },
      \"plugins\": {
        \"key-auth\": {}
      }
    }" | jq -c '{id: .id}' 2>/dev/null || true
  ok "Route created: /bench-auth (with key-auth plugin)"

  # --- Verify routes ---
  local health
  health=$(curl -sf "http://127.0.0.1:${ANDO_ADMIN_PORT}/apisix/admin/health")
  ok "Gateway health: $(echo "${health}" | jq -c '{version, cache, plugins_loaded}')"
}

# ---- Run wrk ----
run_wrk() {
  local url="$1" label="$2" header_arg="${3:-}" conns="${4:-${CONNECTIONS}}"
  local out_file="${RESULTS_DIR}/native_wrk_${label}_${TIMESTAMP}.txt"

  local cmd=(wrk -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency)
  [ -n "${header_arg}" ] && cmd+=(-H "${header_arg}")
  cmd+=("${url}")

  info "Running: ${cmd[*]}"
  "${cmd[@]}" 2>&1 | tee "${out_file}"
  echo "${out_file}"
}

extract_rps() {
  grep -E "Requests/sec" "$1" | awk '{print $2}' | tail -1
}

extract_p99() {
  grep "99%" "$1" | awk '{print $2}' | tail -1
}

# ---- Warm up ----
warmup() {
  local url="$1"
  info "Warming up (10s)..."
  wrk -t 4 -c 50 -d 10s "${url}" > /dev/null 2>&1 || true
  sleep 1
}

# ---- Scenarios ----
PLAIN_RPS="" AUTH_RPS="" STRESS_RPS=""
PLAIN_P99="" AUTH_P99="" STRESS_P99=""
PLAIN_OUT="" AUTH_OUT="" STRESS_OUT=""

bench_plain() {
  header "Scenario 1 — Plain Proxy (${CONNECTIONS} connections, ${DURATION})"
  warmup "http://127.0.0.1:${ANDO_PROXY_PORT}/bench"
  PLAIN_OUT=$(run_wrk "http://127.0.0.1:${ANDO_PROXY_PORT}/bench" "plain")
  PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/native_wrk_plain_${TIMESTAMP}.txt")
  PLAIN_P99=$(extract_p99 "${RESULTS_DIR}/native_wrk_plain_${TIMESTAMP}.txt")
}

bench_auth() {
  header "Scenario 2 — Key-Auth Plugin (${CONNECTIONS} connections, ${DURATION})"
  warmup "http://127.0.0.1:${ANDO_PROXY_PORT}/bench-auth"
  AUTH_OUT=$(run_wrk "http://127.0.0.1:${ANDO_PROXY_PORT}/bench-auth" "auth" "apikey: ${API_KEY}")
  AUTH_RPS=$(extract_rps "${RESULTS_DIR}/native_wrk_auth_${TIMESTAMP}.txt")
  AUTH_P99=$(extract_p99 "${RESULTS_DIR}/native_wrk_auth_${TIMESTAMP}.txt")
}

bench_stress() {
  header "Scenario 3 — Stress Test (${STRESS_CONNECTIONS} connections, ${DURATION})"
  warn "Pushing Ando hard — some errors at saturation are expected"
  warmup "http://127.0.0.1:${ANDO_PROXY_PORT}/bench"
  STRESS_OUT=$(run_wrk "http://127.0.0.1:${ANDO_PROXY_PORT}/bench" "stress" "" "${STRESS_CONNECTIONS}")
  STRESS_RPS=$(extract_rps "${RESULTS_DIR}/native_wrk_stress_${TIMESTAMP}.txt")
  STRESS_P99=$(extract_p99 "${RESULTS_DIR}/native_wrk_stress_${TIMESTAMP}.txt")
}

bench_echo_baseline() {
  header "Scenario 0 — Echo Backend Baseline (no Ando, raw backend)"
  info "Testing echo backend directly to establish ceiling..."
  local out_file="${RESULTS_DIR}/native_wrk_baseline_${TIMESTAMP}.txt"
  wrk -t "${THREADS}" -c "${CONNECTIONS}" -d 15s --latency \
    "http://127.0.0.1:${ECHO_PORT}/" 2>&1 | tee "${out_file}"
  BASELINE_RPS=$(extract_rps "${out_file}")
  BASELINE_P99=$(extract_p99 "${out_file}")
  ok "Backend ceiling: ${BASELINE_RPS} req/s (p99 ${BASELINE_P99})"
}

# ---- Report ----
write_report() {
  local ts
  ts=$(date "+%Y-%m-%d %H:%M:%S %Z")
  local cpu
  cpu=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown")
  local cores
  cores=$(sysctl -n hw.logicalcpu 2>/dev/null || echo "?")

cat > "${REPORT_FILE}" <<EOF
# Ando Native Benchmark Report

**Date**: ${ts}
**Host**: macOS — ${cpu} (${cores} logical cores)
**Tool**: wrk
**Duration**: ${DURATION}
**Threads**: ${THREADS}

---

## Results Summary

| Scenario | Req/sec | p99 Latency |
|---|---|---|
| Backend Baseline (no proxy) | ${BASELINE_RPS:-N/A} | ${BASELINE_P99:-N/A} |
| Plain Proxy (${CONNECTIONS}c) | ${PLAIN_RPS:-N/A} | ${PLAIN_P99:-N/A} |
| Key-Auth Plugin (${CONNECTIONS}c) | ${AUTH_RPS:-N/A} | ${AUTH_P99:-N/A} |
| Stress Test (${STRESS_CONNECTIONS}c) | ${STRESS_RPS:-N/A} | ${STRESS_P99:-N/A} |

---

## Scenario 0 — Backend Baseline
\`\`\`
$(cat "${RESULTS_DIR}/native_wrk_baseline_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")
\`\`\`

## Scenario 1 — Plain Proxy
\`\`\`
$(cat "${RESULTS_DIR}/native_wrk_plain_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")
\`\`\`

## Scenario 2 — Key-Auth Plugin
\`\`\`
$(cat "${RESULTS_DIR}/native_wrk_auth_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")
\`\`\`

## Scenario 3 — Stress Test
\`\`\`
$(cat "${RESULTS_DIR}/native_wrk_stress_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")
\`\`\`

## Ando Startup Log (last 50 lines)
\`\`\`
$(tail -50 "${RESULTS_DIR}/ando-native.log" 2>/dev/null || echo "N/A")
\`\`\`
EOF

  ok "Report: ${REPORT_FILE}"
}

# ---- Main ----
echo -e "\n${BOLD}${CYAN}"
echo "  ╔════════════════════════════════════════════╗"
echo "  ║   Ando Native Benchmark (No Docker)        ║"
echo "  ╚════════════════════════════════════════════╝"
echo -e "${NC}"

info "Scenario: ${SCENARIO}"
info "Duration: ${DURATION}"
info "Connections: ${CONNECTIONS} (stress: ${STRESS_CONNECTIONS})"
info "Threads: ${THREADS}"
echo ""

start_backend
start_ando
setup_routes

BASELINE_RPS="" BASELINE_P99=""

case "${SCENARIO}" in
  all)
    bench_echo_baseline
    bench_plain
    bench_auth
    bench_stress
    ;;
  plain)
    bench_echo_baseline
    bench_plain
    ;;
  auth)
    bench_auth
    ;;
  stress)
    bench_stress
    ;;
  baseline)
    bench_echo_baseline
    ;;
  *)
    err "Unknown scenario. Use: all|plain|auth|stress|baseline"
    exit 1
    ;;
esac

write_report

header "Done!"
echo ""
ok "Results: ${RESULTS_DIR}/"
ok "Report:  ${REPORT_FILE}"
echo ""
echo -e "${CYAN}To view the report:${NC}"
echo "  open ${REPORT_FILE}"
echo "  cat  ${REPORT_FILE}"
echo ""
