#!/usr/bin/env bash
# ============================================================
# bench.sh — Ando v1 vs Ando v2 vs APISIX Benchmark
# ============================================================
# Usage:
#   ./benchmark/bench.sh [baseline|plain|auth|stress|ramp|all]
#
# Env overrides:
#   BENCH_DURATION=60s BENCH_CONNECTIONS=400 ./benchmark/bench.sh
#
# Requires: docker (with compose plugin)
# ============================================================

set -euo pipefail
trap cleanup EXIT

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
RESULTS_DIR="${SCRIPT_DIR}/results/${TIMESTAMP}"
SCENARIO="${1:-all}"

BENCH_NET="bench_net"
WRK_IMAGE="benchmark-wrk:latest"

# ── Params (override via env) ────────────────────────────────
DURATION="${BENCH_DURATION:-30s}"
CONNECTIONS="${BENCH_CONNECTIONS:-200}"
THREADS="${BENCH_THREADS:-4}"
STRESS_CONNECTIONS="${BENCH_STRESS_CONNECTIONS:-500}"
API_KEY="${BENCH_API_KEY:-bench-secret-key}"
REPORT_FILE="${RESULTS_DIR}/report.md"

# ── Colors ────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

mkdir -p "${RESULTS_DIR}"

info()   { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()     { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()   { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()    { echo -e "${RED}[ERR]${NC}   $*"; }
header() {
  echo -e "\n${BOLD}${CYAN}════════════════════════════════════════${NC}"
  echo -e "${BOLD}  $*${NC}"
  echo -e "${BOLD}${CYAN}════════════════════════════════════════${NC}"
}

# ── Cleanup ───────────────────────────────────────────────────
cleanup() {
  echo ""
  info "Stopping services..."
  docker compose -f "${SCRIPT_DIR}/docker-compose.yml" down --remove-orphans 2>/dev/null || true
}

# ── Check docker ──────────────────────────────────────────────
check_docker() {
  if ! command -v docker &>/dev/null; then
    err "Docker not found. Install from: https://docs.docker.com/get-docker/"
    exit 1
  fi
  if ! docker info &>/dev/null; then
    err "Docker daemon is not running. Start Docker Desktop and retry."
    exit 1
  fi
}

# ── Build wrk image (native arch) ─────────────────────────────
build_wrk() {
  if docker image inspect "${WRK_IMAGE}" &>/dev/null; then
    info "wrk image already built"
  else
    info "Building native wrk image..."
    docker build -t "${WRK_IMAGE}" -f "${SCRIPT_DIR}/Dockerfile.wrk" "${SCRIPT_DIR}"
    ok "wrk image built"
  fi
}

# ── Start all services ────────────────────────────────────────
start_services() {
  header "Starting services (first build may take several minutes)"
  docker compose -f "${SCRIPT_DIR}/docker-compose.yml" up -d --build --wait
  ok "All services healthy"
}

# ── Docker curl helper ────────────────────────────────────────
dcurl() {
  docker run --rm --network "${BENCH_NET}" curlimages/curl:latest -sf "$@"
}

# ── Setup routes on all three gateways ───────────────────────
setup_routes() {
  header "Configuring Ando v1 + Ando v2 + APISIX routes"

  # ── Ando v1 ──────────────────────────────────────────────────
  info "Setting up Ando v1 routes..."
  dcurl -X PUT "http://ando-v1:9180/apisix/admin/upstreams/bench-echo" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-echo","type":"roundrobin","nodes":{"echo:3000":1}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  v1 upstream: bench-echo → echo:3000"

  dcurl -X PUT "http://ando-v1:9180/apisix/admin/routes/bench-plain" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-plain","uri":"/bench/plain","methods":["GET"],"upstream_id":"bench-echo","plugins":{}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  v1 route: /bench/plain (no auth)"

  dcurl -X PUT "http://ando-v1:9180/apisix/admin/consumers/bench-user" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"bench-user\",\"plugins\":{\"key-auth\":{\"key\":\"${API_KEY}\"}}}" \
    | grep -o '"username":"[^"]*"' || true

  dcurl -X PUT "http://ando-v1:9180/apisix/admin/routes/bench-auth" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-auth","uri":"/bench/auth","methods":["GET"],"upstream_id":"bench-echo","plugins":{"key-auth":{}}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  v1 route: /bench/auth (key-auth)"

  # ── Ando v2 ──────────────────────────────────────────────────
  info "Setting up Ando v2 routes..."
  dcurl -X PUT "http://ando-v2:9180/apisix/admin/upstreams/bench-echo" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-echo","type":"roundrobin","nodes":{"echo:3000":1}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  v2 upstream: bench-echo → echo:3000"

  dcurl -X PUT "http://ando-v2:9180/apisix/admin/routes/bench-plain" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-plain","uri":"/bench/plain","methods":["GET"],"upstream_id":"bench-echo","plugins":{}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  v2 route: /bench/plain (no auth)"

  dcurl -X PUT "http://ando-v2:9180/apisix/admin/consumers/bench-user" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"bench-user\",\"plugins\":{\"key-auth\":{\"key\":\"${API_KEY}\"}}}" \
    | grep -o '"username":"[^"]*"' || true

  dcurl -X PUT "http://ando-v2:9180/apisix/admin/routes/bench-auth" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-auth","uri":"/bench/auth","methods":["GET"],"upstream_id":"bench-echo","plugins":{"key-auth":{}}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  v2 route: /bench/auth (key-auth)"

  # ── APISIX ───────────────────────────────────────────────────
  info "Waiting for APISIX admin API..."
  for i in {1..30}; do
    dcurl -H "X-API-KEY: bench-admin-key-00000000000000" http://apisix:9180/apisix/admin/routes >/dev/null 2>&1 && break
    sleep 2
  done

  dcurl -X PUT "http://apisix:9180/apisix/admin/upstreams/1" \
    -H "X-API-KEY: bench-admin-key-00000000000000" \
    -H "Content-Type: application/json" \
    -d '{"id":"1","type":"roundrobin","nodes":[{"host":"echo","port":3000,"weight":1}]}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  APISIX upstream → echo:3000"

  dcurl -X PUT "http://apisix:9180/apisix/admin/routes/1" \
    -H "X-API-KEY: bench-admin-key-00000000000000" \
    -H "Content-Type: application/json" \
    -d '{"id":"1","uri":"/bench/plain","methods":["GET"],"upstream_id":"1","plugins":{}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  APISIX route: /bench/plain (no auth)"

  dcurl -X PUT "http://apisix:9180/apisix/admin/consumers/bench-user" \
    -H "X-API-KEY: bench-admin-key-00000000000000" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"bench-user\",\"plugins\":{\"key-auth\":{\"key\":\"${API_KEY}\"}}}" \
    | grep -o '"username":"[^"]*"' || true

  dcurl -X PUT "http://apisix:9180/apisix/admin/routes/2" \
    -H "X-API-KEY: bench-admin-key-00000000000000" \
    -H "Content-Type: application/json" \
    -d '{"id":"2","uri":"/bench/auth","methods":["GET"],"upstream_id":"1","plugins":{"key-auth":{}}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  APISIX route: /bench/auth (key-auth)"
}

# ============================================================
# Wrk helpers
# ============================================================

warmup() {
  local url="$1" name="$2" extra_header="${3:-}"
  info "Warming up ${name} (10s)..."
  local cmd=("${WRK_IMAGE}" -t 4 -c 50 -d 10s)
  [ -n "${extra_header}" ] && cmd+=(-H "${extra_header}")
  cmd+=("${url}")
  docker run --rm --network "${BENCH_NET}" "${cmd[@]}" >/dev/null 2>&1 || warn "Warmup failed (continuing anyway)"
}

run_wrk() {
  local url="$1" label="$2" extra_header="${3:-}" conns="${4:-${CONNECTIONS}}"
  local out="${RESULTS_DIR}/wrk_${label}.txt"
  local cmd=("${WRK_IMAGE}" -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency)
  [ -n "${extra_header}" ] && cmd+=(-H "${extra_header}")
  cmd+=("${url}")
  docker run --rm --network "${BENCH_NET}" "${cmd[@]}" 2>&1 | tee "${out}"
}

extract_rps() { grep -E "Req/Sec|Requests/sec" "$1" | awk '{print $2}' | tail -1; }
extract_p99() { grep "99%" "$1" | awk '{print $2}' | tail -1; }

# ── Result variables ──────────────────────────────────────────
BASELINE_RPS="" BASELINE_P99=""
V1_PLAIN_RPS=""  V1_PLAIN_P99=""
V2_PLAIN_RPS=""  V2_PLAIN_P99=""
APISIX_PLAIN_RPS="" APISIX_PLAIN_P99=""
V1_AUTH_RPS=""   V1_AUTH_P99=""
V2_AUTH_RPS=""   V2_AUTH_P99=""
APISIX_AUTH_RPS=""  APISIX_AUTH_P99=""
V1_STRESS_RPS=""    V1_STRESS_P99=""
V2_STRESS_RPS=""    V2_STRESS_P99=""
APISIX_STRESS_RPS="" APISIX_STRESS_P99=""
RAMP_V1_RPS=()   RAMP_V1_P99=()
RAMP_V2_RPS=()   RAMP_V2_P99=()
RAMP_APISIX_RPS=() RAMP_APISIX_P99=()

# ============================================================
# Scenarios
# ============================================================

bench_baseline() {
  header "Scenario 0 — Echo Backend Baseline (no proxy)"
  warmup "http://echo:3000/" "echo"
  run_wrk "http://echo:3000/" "baseline"
  BASELINE_RPS=$(extract_rps "${RESULTS_DIR}/wrk_baseline.txt")
  BASELINE_P99=$(extract_p99 "${RESULTS_DIR}/wrk_baseline.txt")
  ok "Baseline: ${BASELINE_RPS:-?} req/s  p99 ${BASELINE_P99:-?}"
}

bench_plain() {
  header "Scenario 1 — Plain Proxy (${CONNECTIONS} conns, ${DURATION})"

  warmup "http://ando-v1:9080/bench/plain" "Ando v1"
  run_wrk "http://ando-v1:9080/bench/plain" "v1_plain"
  V1_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_v1_plain.txt")
  V1_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_v1_plain.txt")
  ok "Ando v1 plain: ${V1_PLAIN_RPS:-?} req/s  p99 ${V1_PLAIN_P99:-?}"

  warmup "http://ando-v2:9080/bench/plain" "Ando v2"
  run_wrk "http://ando-v2:9080/bench/plain" "v2_plain"
  V2_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_v2_plain.txt")
  V2_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_v2_plain.txt")
  ok "Ando v2 plain: ${V2_PLAIN_RPS:-?} req/s  p99 ${V2_PLAIN_P99:-?}"

  warmup "http://apisix:8080/bench/plain" "APISIX"
  run_wrk "http://apisix:8080/bench/plain" "apisix_plain"
  APISIX_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_plain.txt")
  APISIX_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_apisix_plain.txt")
  ok "APISIX plain:  ${APISIX_PLAIN_RPS:-?} req/s  p99 ${APISIX_PLAIN_P99:-?}"
}

bench_auth() {
  header "Scenario 2 — Key-Auth Plugin (${CONNECTIONS} conns, ${DURATION})"

  warmup "http://ando-v1:9080/bench/auth" "Ando v1 auth" "apikey: ${API_KEY}"
  run_wrk "http://ando-v1:9080/bench/auth" "v1_auth" "apikey: ${API_KEY}"
  V1_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_v1_auth.txt")
  V1_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_v1_auth.txt")
  ok "Ando v1 auth: ${V1_AUTH_RPS:-?} req/s  p99 ${V1_AUTH_P99:-?}"

  warmup "http://ando-v2:9080/bench/auth" "Ando v2 auth" "apikey: ${API_KEY}"
  run_wrk "http://ando-v2:9080/bench/auth" "v2_auth" "apikey: ${API_KEY}"
  V2_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_v2_auth.txt")
  V2_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_v2_auth.txt")
  ok "Ando v2 auth: ${V2_AUTH_RPS:-?} req/s  p99 ${V2_AUTH_P99:-?}"

  warmup "http://apisix:8080/bench/auth" "APISIX auth" "apikey: ${API_KEY}"
  run_wrk "http://apisix:8080/bench/auth" "apisix_auth" "apikey: ${API_KEY}"
  APISIX_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_auth.txt")
  APISIX_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_apisix_auth.txt")
  ok "APISIX auth:  ${APISIX_AUTH_RPS:-?} req/s  p99 ${APISIX_AUTH_P99:-?}"
}

bench_stress() {
  header "Scenario 3 — Stress Test (${STRESS_CONNECTIONS} conns, ${DURATION})"
  warn "High concurrency — some errors at saturation are expected."

  warmup "http://ando-v1:9080/bench/plain" "Ando v1"
  run_wrk "http://ando-v1:9080/bench/plain" "v1_stress" "" "${STRESS_CONNECTIONS}"
  V1_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_v1_stress.txt")
  V1_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_v1_stress.txt")
  ok "Ando v1 stress: ${V1_STRESS_RPS:-?} req/s  p99 ${V1_STRESS_P99:-?}"

  warmup "http://ando-v2:9080/bench/plain" "Ando v2"
  run_wrk "http://ando-v2:9080/bench/plain" "v2_stress" "" "${STRESS_CONNECTIONS}"
  V2_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_v2_stress.txt")
  V2_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_v2_stress.txt")
  ok "Ando v2 stress: ${V2_STRESS_RPS:-?} req/s  p99 ${V2_STRESS_P99:-?}"

  warmup "http://apisix:8080/bench/plain" "APISIX"
  run_wrk "http://apisix:8080/bench/plain" "apisix_stress" "" "${STRESS_CONNECTIONS}"
  APISIX_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_stress.txt")
  APISIX_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_apisix_stress.txt")
  ok "APISIX stress:  ${APISIX_STRESS_RPS:-?} req/s  p99 ${APISIX_STRESS_P99:-?}"
}

bench_ramp() {
  header "Scenario 4 — Concurrency Ramp (10 → 1000)"
  local RAMP_CONNS=(10 50 100 250 500 1000)
  local old_dur="${DURATION}"; DURATION="15s"

  printf "\n%-8s %-18s %-14s %-18s %-14s %-18s %-14s\n" \
    "Conns" "v1 req/s" "v1 p99" "v2 req/s" "v2 p99" "APISIX req/s" "APISIX p99"
  printf '%0.s─' {1..100}; echo ""

  for conns in "${RAMP_CONNS[@]}"; do
    local v1o="${RESULTS_DIR}/wrk_ramp_v1_${conns}.txt"
    local v2o="${RESULTS_DIR}/wrk_ramp_v2_${conns}.txt"
    local co="${RESULTS_DIR}/wrk_ramp_apisix_${conns}.txt"

    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://ando-v1:9080/bench/plain > "${v1o}" 2>&1 || true
    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://ando-v2:9080/bench/plain > "${v2o}" 2>&1 || true
    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://apisix:8080/bench/plain > "${co}" 2>&1 || true

    local v1r v1p v2r v2p cr cp
    v1r=$(extract_rps "${v1o}" || echo 0); v1p=$(extract_p99 "${v1o}" || echo "N/A")
    v2r=$(extract_rps "${v2o}" || echo 0); v2p=$(extract_p99 "${v2o}" || echo "N/A")
    cr=$(extract_rps  "${co}"  || echo 0); cp=$(extract_p99  "${co}"  || echo "N/A")
    RAMP_V1_RPS+=("${v1r}"); RAMP_V1_P99+=("${v1p}")
    RAMP_V2_RPS+=("${v2r}"); RAMP_V2_P99+=("${v2p}")
    RAMP_APISIX_RPS+=("${cr}"); RAMP_APISIX_P99+=("${cp}")

    printf "%-8s %-18s %-14s %-18s %-14s %-18s %-14s\n" \
      "${conns}" "${v1r}" "${v1p}" "${v2r}" "${v2p}" "${cr}" "${cp}"
  done
  DURATION="${old_dur}"
}

# ============================================================
# Report
# ============================================================
write_report() {
  local ts cpu
  ts=$(date "+%Y-%m-%d %H:%M:%S %Z")
  cpu=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || \
        grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo "unknown")

  to_int() { echo "${1:-0}" | sed 's/[^0-9.]//g' | awk '{printf "%d",$1+0}'; }
  to_ms()  { echo "${1:-0}" | sed 's/ms//' | awk '{printf "%.2f",$1+0}'; }

  local v1_plain;  v1_plain=$(to_int  "${V1_PLAIN_RPS}")
  local v2_plain;  v2_plain=$(to_int  "${V2_PLAIN_RPS}")
  local c_plain;   c_plain=$(to_int   "${APISIX_PLAIN_RPS}")
  local v1_auth;   v1_auth=$(to_int   "${V1_AUTH_RPS}")
  local v2_auth;   v2_auth=$(to_int   "${V2_AUTH_RPS}")
  local c_auth;    c_auth=$(to_int    "${APISIX_AUTH_RPS}")
  local v1_stress; v1_stress=$(to_int "${V1_STRESS_RPS}")
  local v2_stress; v2_stress=$(to_int "${V2_STRESS_RPS}")
  local c_stress;  c_stress=$(to_int  "${APISIX_STRESS_RPS}")

  local v1_plain_ms; v1_plain_ms=$(to_ms "${V1_PLAIN_P99}")
  local v2_plain_ms; v2_plain_ms=$(to_ms "${V2_PLAIN_P99}")
  local c_plain_ms;  c_plain_ms=$(to_ms  "${APISIX_PLAIN_P99}")
  local v1_auth_ms;  v1_auth_ms=$(to_ms  "${V1_AUTH_P99}")
  local v2_auth_ms;  v2_auth_ms=$(to_ms  "${V2_AUTH_P99}")
  local c_auth_ms;   c_auth_ms=$(to_ms   "${APISIX_AUTH_P99}")
  local v1_stress_ms; v1_stress_ms=$(to_ms "${V1_STRESS_P99}")
  local v2_stress_ms; v2_stress_ms=$(to_ms "${V2_STRESS_P99}")
  local c_stress_ms;  c_stress_ms=$(to_ms  "${APISIX_STRESS_P99}")

  # Ramp CSV
  local ramp_v1_rps_csv="" ramp_v2_rps_csv="" ramp_apisix_rps_csv=""
  local ramp_v1_p99_csv="" ramp_v2_p99_csv="" ramp_apisix_p99_csv=""
  if [ ${#RAMP_V1_RPS[@]} -gt 0 ]; then
    local tmp=""
    for v in "${RAMP_V1_RPS[@]}";    do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done; ramp_v1_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_V2_RPS[@]}";    do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done; ramp_v2_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_APISIX_RPS[@]}";do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done; ramp_apisix_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_V1_P99[@]}";    do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done; ramp_v1_p99_csv="${tmp}"; tmp=""
    for v in "${RAMP_V2_P99[@]}";    do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done; ramp_v2_p99_csv="${tmp}"; tmp=""
    for v in "${RAMP_APISIX_P99[@]}";do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done; ramp_apisix_p99_csv="${tmp}"
  fi

  winner3() {
    local a="${1:-0}" b="${2:-0}" c="${3:-0}"
    local ai bi ci
    ai=$(echo "${a}" | awk '{printf "%d",$1*1000}' 2>/dev/null || echo 0)
    bi=$(echo "${b}" | awk '{printf "%d",$1*1000}' 2>/dev/null || echo 0)
    ci=$(echo "${c}" | awk '{printf "%d",$1*1000}' 2>/dev/null || echo 0)
    local max=$((ai > bi ? ai : bi)); max=$((max > ci ? max : ci))
    local winners=""
    [[ "${ai}" == "${max}" ]] && winners="${winners:+${winners}/}**Ando v1**"
    [[ "${bi}" == "${max}" ]] && winners="${winners:+${winners}/}**Ando v2**"
    [[ "${ci}" == "${max}" ]] && winners="${winners:+${winners}/}**APISIX**"
    echo "${winners:-tie}"
  }

  cat > "${REPORT_FILE}" <<EOF
# Ando v1 vs Ando v2 vs APISIX — Benchmark Report

**Date**        : ${ts}
**Host**        : ${cpu}
**Duration**    : ${DURATION} per scenario
**Threads**     : ${THREADS}
**Connections** : ${CONNECTIONS}  (stress: ${STRESS_CONNECTIONS})
**Run folder**  : \`$(basename "${RESULTS_DIR}")\`

---

## Throughput — Requests per Second (higher is better)

> **Bar 1 = Ando v1** &nbsp;|&nbsp; **Bar 2 = Ando v2** &nbsp;|&nbsp; **Bar 3 = APISIX**

\`\`\`mermaid
xychart-beta
    title "Throughput — Requests per Second"
    x-axis ["Plain Proxy", "Key-Auth", "Stress (${STRESS_CONNECTIONS}c)"]
    y-axis "req/s"
    bar [${v1_plain}, ${v1_auth}, ${v1_stress}]
    bar [${v2_plain}, ${v2_auth}, ${v2_stress}]
    bar [${c_plain},  ${c_auth},  ${c_stress}]
\`\`\`

## p99 Latency — ms (lower is better)

> **Bar 1 = Ando v1** &nbsp;|&nbsp; **Bar 2 = Ando v2** &nbsp;|&nbsp; **Bar 3 = APISIX**

\`\`\`mermaid
xychart-beta
    title "p99 Latency (ms)"
    x-axis ["Plain Proxy", "Key-Auth", "Stress (${STRESS_CONNECTIONS}c)"]
    y-axis "latency ms"
    bar [${v1_plain_ms}, ${v1_auth_ms}, ${v1_stress_ms}]
    bar [${v2_plain_ms}, ${v2_auth_ms}, ${v2_stress_ms}]
    bar [${c_plain_ms},  ${c_auth_ms},  ${c_stress_ms}]
\`\`\`

---

## Three-Way Comparison

| Scenario | Ando v1 req/s | v1 p99 | Ando v2 req/s | v2 p99 | APISIX req/s | APISIX p99 | Winner |
|---|---|---|---|---|---|---|---|
| Plain Proxy (${CONNECTIONS}c) | ${V1_PLAIN_RPS:-N/A} | ${V1_PLAIN_P99:-N/A} | ${V2_PLAIN_RPS:-N/A} | ${V2_PLAIN_P99:-N/A} | ${APISIX_PLAIN_RPS:-N/A} | ${APISIX_PLAIN_P99:-N/A} | $(winner3 "${V1_PLAIN_RPS:-0}" "${V2_PLAIN_RPS:-0}" "${APISIX_PLAIN_RPS:-0}") |
| Key-Auth (${CONNECTIONS}c) | ${V1_AUTH_RPS:-N/A} | ${V1_AUTH_P99:-N/A} | ${V2_AUTH_RPS:-N/A} | ${V2_AUTH_P99:-N/A} | ${APISIX_AUTH_RPS:-N/A} | ${APISIX_AUTH_P99:-N/A} | $(winner3 "${V1_AUTH_RPS:-0}" "${V2_AUTH_RPS:-0}" "${APISIX_AUTH_RPS:-0}") |
| Stress Test (${STRESS_CONNECTIONS}c) | ${V1_STRESS_RPS:-N/A} | ${V1_STRESS_P99:-N/A} | ${V2_STRESS_RPS:-N/A} | ${V2_STRESS_P99:-N/A} | ${APISIX_STRESS_RPS:-N/A} | ${APISIX_STRESS_P99:-N/A} | $(winner3 "${V1_STRESS_RPS:-0}" "${V2_STRESS_RPS:-0}" "${APISIX_STRESS_RPS:-0}") |

---

## Summary

| Scenario | req/s | p99 |
|---|---|---|
| Baseline (echo, no proxy) | ${BASELINE_RPS:-N/A} | ${BASELINE_P99:-N/A} |
| Ando v1 plain proxy | ${V1_PLAIN_RPS:-N/A} | ${V1_PLAIN_P99:-N/A} |
| Ando v2 plain proxy | ${V2_PLAIN_RPS:-N/A} | ${V2_PLAIN_P99:-N/A} |
| Ando v1 key-auth | ${V1_AUTH_RPS:-N/A} | ${V1_AUTH_P99:-N/A} |
| Ando v2 key-auth | ${V2_AUTH_RPS:-N/A} | ${V2_AUTH_P99:-N/A} |
| Ando v1 stress (${STRESS_CONNECTIONS}c) | ${V1_STRESS_RPS:-N/A} | ${V1_STRESS_P99:-N/A} |
| Ando v2 stress (${STRESS_CONNECTIONS}c) | ${V2_STRESS_RPS:-N/A} | ${V2_STRESS_P99:-N/A} |
| APISIX plain proxy | ${APISIX_PLAIN_RPS:-N/A} | ${APISIX_PLAIN_P99:-N/A} |
| APISIX key-auth | ${APISIX_AUTH_RPS:-N/A} | ${APISIX_AUTH_P99:-N/A} |
| APISIX stress (${STRESS_CONNECTIONS}c) | ${APISIX_STRESS_RPS:-N/A} | ${APISIX_STRESS_P99:-N/A} |

EOF

  # Ramp section
  if [ -n "${ramp_v1_rps_csv}" ]; then
    cat >> "${REPORT_FILE}" <<EOF
---

## Scenario 4 — Concurrency Ramp (10 → 1000 connections)

> **Line 1 = Ando v1** &nbsp;|&nbsp; **Line 2 = Ando v2** &nbsp;|&nbsp; **Line 3 = APISIX**

\`\`\`mermaid
xychart-beta
    title "Concurrency Ramp — Requests per Second"
    x-axis ["10c", "50c", "100c", "250c", "500c", "1000c"]
    y-axis "req/s"
    line [${ramp_v1_rps_csv}]
    line [${ramp_v2_rps_csv}]
    line [${ramp_apisix_rps_csv}]
\`\`\`

\`\`\`mermaid
xychart-beta
    title "Concurrency Ramp — p99 Latency (ms, lower is better)"
    x-axis ["10c", "50c", "100c", "250c", "500c", "1000c"]
    y-axis "latency ms"
    line [${ramp_v1_p99_csv}]
    line [${ramp_v2_p99_csv}]
    line [${ramp_apisix_p99_csv}]
\`\`\`

### Ramp Data Table

| Conns | v1 req/s | v1 p99 | v2 req/s | v2 p99 | APISIX req/s | APISIX p99 |
|---|---|---|---|---|---|---|
EOF
    local RAMP_CONNS=(10 50 100 250 500 1000)
    for i in "${!RAMP_CONNS[@]}"; do
      printf "| %s | %s | %s | %s | %s | %s | %s |\n" \
        "${RAMP_CONNS[$i]}" \
        "${RAMP_V1_RPS[$i]:-N/A}"    "${RAMP_V1_P99[$i]:-N/A}" \
        "${RAMP_V2_RPS[$i]:-N/A}"    "${RAMP_V2_P99[$i]:-N/A}" \
        "${RAMP_APISIX_RPS[$i]:-N/A}" "${RAMP_APISIX_P99[$i]:-N/A}" \
        >> "${REPORT_FILE}"
    done
    echo "" >> "${REPORT_FILE}"
  fi

  # Raw outputs
  cat >> "${REPORT_FILE}" <<EOF
---

## Raw wrk Outputs

### Baseline
\`\`\`
$(cat "${RESULTS_DIR}/wrk_baseline.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Plain Proxy — Ando v1
\`\`\`
$(cat "${RESULTS_DIR}/wrk_v1_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Plain Proxy — Ando v2
\`\`\`
$(cat "${RESULTS_DIR}/wrk_v2_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Plain Proxy — APISIX
\`\`\`
$(cat "${RESULTS_DIR}/wrk_apisix_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Key-Auth — Ando v1
\`\`\`
$(cat "${RESULTS_DIR}/wrk_v1_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Key-Auth — Ando v2
\`\`\`
$(cat "${RESULTS_DIR}/wrk_v2_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Key-Auth — APISIX
\`\`\`
$(cat "${RESULTS_DIR}/wrk_apisix_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Stress Test — Ando v1
\`\`\`
$(cat "${RESULTS_DIR}/wrk_v1_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Stress Test — Ando v2
\`\`\`
$(cat "${RESULTS_DIR}/wrk_v2_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Stress Test — APISIX
\`\`\`
$(cat "${RESULTS_DIR}/wrk_apisix_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`
EOF

  ok "Report: ${REPORT_FILE}"
}

# ============================================================
# Entry point
# ============================================================
echo -e "\n${BOLD}${CYAN}"
echo "  ╔═════════════════════════════════════════════════╗"
echo "  ║   Ando v1 vs Ando v2 vs APISIX — Benchmark     ║"
echo "  ╚═════════════════════════════════════════════════╝"
echo -e "${NC}"

info "Scenario  : ${SCENARIO}"
info "Duration  : ${DURATION}"
info "Conns     : ${CONNECTIONS}  (stress: ${STRESS_CONNECTIONS})"
info "Run dir   : ${RESULTS_DIR}"
echo ""
echo -e "${YELLOW}Tip: BENCH_DURATION=60s BENCH_CONNECTIONS=400 $0 all${NC}"
echo ""

check_docker
build_wrk
start_services
setup_routes

case "${SCENARIO}" in
  all)
    bench_baseline; bench_plain; bench_auth; bench_stress; bench_ramp
    write_report ;;
  baseline)
    bench_baseline; write_report ;;
  plain)
    bench_baseline; bench_plain; write_report ;;
  auth)
    bench_auth; write_report ;;
  stress)
    bench_stress; write_report ;;
  ramp)
    bench_ramp; write_report ;;
  *)
    err "Unknown scenario: ${SCENARIO}"
    echo "Usage: $0 [all|baseline|plain|auth|stress|ramp]"
    exit 1 ;;
esac

header "Done!"
ok "Results: ${RESULTS_DIR}/"
[ -f "${REPORT_FILE}" ] && echo "  open ${REPORT_FILE}"
echo ""
