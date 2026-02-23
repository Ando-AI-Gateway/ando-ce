#!/usr/bin/env bash
# ============================================================
# bench.sh — Ando CE vs APISIX vs KrakenD vs Kong vs Tyk
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
  ok "All services with healthchecks are healthy"

  # Tyk is distroless (no in-container curl); poll it externally.
  info "Waiting for Tyk Gateway to be ready..."
  local tries=0
  until docker run --rm --network "${BENCH_NET}" curlimages/curl:latest \
      -sf http://tyk:8080/hello >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ $tries -ge 30 ]; then
      err "Tyk Gateway did not become ready after 60s"
      exit 1
    fi
    sleep 2
  done
  ok "Tyk Gateway ready"
}

# ── Docker curl helper ────────────────────────────────────────
dcurl() {
  docker run --rm --network "${BENCH_NET}" curlimages/curl:latest -sf "$@"
}

# ── Setup routes on all three gateways ───────────────────────
setup_routes() {
  header "Configuring Ando CE + APISIX routes"

  # ── Ando CE (monoio / thread-per-core) ───────────────────────
  info "Setting up Ando CE routes..."
  dcurl -X PUT "http://ando-ce:9180/apisix/admin/upstreams/bench-echo" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-echo","type":"roundrobin","nodes":{"echo:3000":1}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  CE upstream: bench-echo → echo:3000"

  dcurl -X PUT "http://ando-ce:9180/apisix/admin/routes/bench-plain" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-plain","uri":"/bench/plain","methods":["GET"],"upstream_id":"bench-echo","plugins":{}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  CE route: /bench/plain (no auth)"

  dcurl -X PUT "http://ando-ce:9180/apisix/admin/consumers/bench-user" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"bench-user\",\"plugins\":{\"key-auth\":{\"key\":\"${API_KEY}\"}}}" \
    | grep -o '"username":"[^"]*"' || true

  dcurl -X PUT "http://ando-ce:9180/apisix/admin/routes/bench-auth" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-auth","uri":"/bench/auth","methods":["GET"],"upstream_id":"bench-echo","plugins":{"key-auth":{}}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "  CE route: /bench/auth (key-auth)"

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

  # ── KrakenD (config-file based, no runtime setup needed) ─────
  info "KrakenD routes are defined in krakend.json — no runtime setup required"
  ok "  KrakenD: /bench/plain (no auth), /bench/auth (header CEL check)"

  # ── Kong (declarative config loaded at startup) ───────────────
  info "Kong routes are defined in kong.yml — verifying admin API..."
  for i in {1..30}; do
    dcurl http://kong:8001/status >/dev/null 2>&1 && break
    sleep 2
  done
  ok "  Kong: /bench/plain (no auth), /bench/auth (key-auth plugin)"

  # ── Tyk (create auth key via admin API) ──────────────────────
  info "Setting up Tyk auth key..."
  for i in {1..30}; do
    dcurl -H "x-tyk-authorization: bench-tyk-secret" http://tyk:8080/tyk/apis/ >/dev/null 2>&1 && break
    sleep 2
  done

  dcurl -X POST "http://tyk:8080/tyk/keys/bench-secret-key" \
    -H "x-tyk-authorization: bench-tyk-secret" \
    -H "Content-Type: application/json" \
    -d '{
      "alias": "bench-user",
      "org_id": "",
      "access_rights": {
        "bench-auth": {
          "api_id": "bench-auth",
          "api_name": "Bench Auth",
          "versions": ["Default"]
        }
      },
      "expires": -1
    }' | grep -o '"key":"[^"]*"' || true
  ok "  Tyk: /bench/plain (keyless), /bench/auth (key: bench-secret-key)"
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
CE_PLAIN_RPS=""      CE_PLAIN_P99=""
APISIX_PLAIN_RPS=""  APISIX_PLAIN_P99=""
KRAKEND_PLAIN_RPS="" KRAKEND_PLAIN_P99=""
KONG_PLAIN_RPS=""    KONG_PLAIN_P99=""
TYK_PLAIN_RPS=""     TYK_PLAIN_P99=""
CE_AUTH_RPS=""       CE_AUTH_P99=""
APISIX_AUTH_RPS=""   APISIX_AUTH_P99=""
KRAKEND_AUTH_RPS=""  KRAKEND_AUTH_P99=""
KONG_AUTH_RPS=""     KONG_AUTH_P99=""
TYK_AUTH_RPS=""      TYK_AUTH_P99=""
CE_STRESS_RPS=""     CE_STRESS_P99=""
APISIX_STRESS_RPS="" APISIX_STRESS_P99=""
KRAKEND_STRESS_RPS="" KRAKEND_STRESS_P99=""
KONG_STRESS_RPS=""   KONG_STRESS_P99=""
TYK_STRESS_RPS=""    TYK_STRESS_P99=""
RAMP_CE_RPS=()      RAMP_CE_P99=()
RAMP_APISIX_RPS=()  RAMP_APISIX_P99=()
RAMP_KRAKEND_RPS=() RAMP_KRAKEND_P99=()
RAMP_KONG_RPS=()    RAMP_KONG_P99=()
RAMP_TYK_RPS=()     RAMP_TYK_P99=()

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

  warmup "http://ando-ce:9080/bench/plain" "Ando CE"
  run_wrk "http://ando-ce:9080/bench/plain" "ce_plain"
  CE_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_ce_plain.txt")
  CE_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_ce_plain.txt")
  ok "Ando CE plain: ${CE_PLAIN_RPS:-?} req/s  p99 ${CE_PLAIN_P99:-?}"

  warmup "http://apisix:8080/bench/plain" "APISIX"
  run_wrk "http://apisix:8080/bench/plain" "apisix_plain"
  APISIX_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_plain.txt")
  APISIX_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_apisix_plain.txt")
  ok "APISIX plain:   ${APISIX_PLAIN_RPS:-?} req/s  p99 ${APISIX_PLAIN_P99:-?}"

  warmup "http://krakend:8080/bench/plain" "KrakenD"
  run_wrk "http://krakend:8080/bench/plain" "krakend_plain"
  KRAKEND_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_krakend_plain.txt")
  KRAKEND_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_krakend_plain.txt")
  ok "KrakenD plain:  ${KRAKEND_PLAIN_RPS:-?} req/s  p99 ${KRAKEND_PLAIN_P99:-?}"

  warmup "http://kong:8000/bench/plain" "Kong"
  run_wrk "http://kong:8000/bench/plain" "kong_plain"
  KONG_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_kong_plain.txt")
  KONG_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_kong_plain.txt")
  ok "Kong plain:     ${KONG_PLAIN_RPS:-?} req/s  p99 ${KONG_PLAIN_P99:-?}"

  warmup "http://tyk:8080/bench/plain" "Tyk"
  run_wrk "http://tyk:8080/bench/plain" "tyk_plain"
  TYK_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_tyk_plain.txt")
  TYK_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_tyk_plain.txt")
  ok "Tyk plain:      ${TYK_PLAIN_RPS:-?} req/s  p99 ${TYK_PLAIN_P99:-?}"
}

bench_auth() {
  header "Scenario 2 — Key-Auth Plugin (${CONNECTIONS} conns, ${DURATION})"

  warmup "http://ando-ce:9080/bench/auth" "Ando CE auth" "apikey: ${API_KEY}"
  run_wrk "http://ando-ce:9080/bench/auth" "ce_auth" "apikey: ${API_KEY}"
  CE_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_ce_auth.txt")
  CE_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_ce_auth.txt")
  ok "Ando CE auth: ${CE_AUTH_RPS:-?} req/s  p99 ${CE_AUTH_P99:-?}"

  warmup "http://apisix:8080/bench/auth" "APISIX auth" "apikey: ${API_KEY}"
  run_wrk "http://apisix:8080/bench/auth" "apisix_auth" "apikey: ${API_KEY}"
  APISIX_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_auth.txt")
  APISIX_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_apisix_auth.txt")
  ok "APISIX auth:   ${APISIX_AUTH_RPS:-?} req/s  p99 ${APISIX_AUTH_P99:-?}"

  warmup "http://krakend:8080/bench/auth" "KrakenD auth" "Apikey: ${API_KEY}"
  run_wrk "http://krakend:8080/bench/auth" "krakend_auth" "Apikey: ${API_KEY}"
  KRAKEND_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_krakend_auth.txt")
  KRAKEND_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_krakend_auth.txt")
  ok "KrakenD auth:  ${KRAKEND_AUTH_RPS:-?} req/s  p99 ${KRAKEND_AUTH_P99:-?}"

  warmup "http://kong:8000/bench/auth" "Kong auth" "apikey: ${API_KEY}"
  run_wrk "http://kong:8000/bench/auth" "kong_auth" "apikey: ${API_KEY}"
  KONG_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_kong_auth.txt")
  KONG_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_kong_auth.txt")
  ok "Kong auth:     ${KONG_AUTH_RPS:-?} req/s  p99 ${KONG_AUTH_P99:-?}"

  warmup "http://tyk:8080/bench/auth" "Tyk auth" "apikey: ${API_KEY}"
  run_wrk "http://tyk:8080/bench/auth" "tyk_auth" "apikey: ${API_KEY}"
  TYK_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_tyk_auth.txt")
  TYK_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_tyk_auth.txt")
  ok "Tyk auth:      ${TYK_AUTH_RPS:-?} req/s  p99 ${TYK_AUTH_P99:-?}"
}

bench_stress() {
  header "Scenario 3 — Stress Test (${STRESS_CONNECTIONS} conns, ${DURATION})"
  warn "High concurrency — some errors at saturation are expected."

  warmup "http://ando-ce:9080/bench/plain" "Ando CE"
  run_wrk "http://ando-ce:9080/bench/plain" "ce_stress" "" "${STRESS_CONNECTIONS}"
  CE_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_ce_stress.txt")
  CE_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_ce_stress.txt")
  ok "Ando CE stress: ${CE_STRESS_RPS:-?} req/s  p99 ${CE_STRESS_P99:-?}"

  warmup "http://apisix:8080/bench/plain" "APISIX"
  run_wrk "http://apisix:8080/bench/plain" "apisix_stress" "" "${STRESS_CONNECTIONS}"
  APISIX_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_stress.txt")
  APISIX_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_apisix_stress.txt")
  ok "APISIX stress:  ${APISIX_STRESS_RPS:-?} req/s  p99 ${APISIX_STRESS_P99:-?}"

  warmup "http://krakend:8080/bench/plain" "KrakenD"
  run_wrk "http://krakend:8080/bench/plain" "krakend_stress" "" "${STRESS_CONNECTIONS}"
  KRAKEND_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_krakend_stress.txt")
  KRAKEND_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_krakend_stress.txt")
  ok "KrakenD stress: ${KRAKEND_STRESS_RPS:-?} req/s  p99 ${KRAKEND_STRESS_P99:-?}"

  warmup "http://kong:8000/bench/plain" "Kong"
  run_wrk "http://kong:8000/bench/plain" "kong_stress" "" "${STRESS_CONNECTIONS}"
  KONG_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_kong_stress.txt")
  KONG_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_kong_stress.txt")
  ok "Kong stress:    ${KONG_STRESS_RPS:-?} req/s  p99 ${KONG_STRESS_P99:-?}"

  warmup "http://tyk:8080/bench/plain" "Tyk"
  run_wrk "http://tyk:8080/bench/plain" "tyk_stress" "" "${STRESS_CONNECTIONS}"
  TYK_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_tyk_stress.txt")
  TYK_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_tyk_stress.txt")
  ok "Tyk stress:     ${TYK_STRESS_RPS:-?} req/s  p99 ${TYK_STRESS_P99:-?}"
}

bench_ramp() {
  header "Scenario 4 — Concurrency Ramp (10 → 1000)"
  local RAMP_CONNS=(10 50 100 250 500 1000)
  local old_dur="${DURATION}"; DURATION="15s"

  printf "\n%-8s %-14s %-14s %-14s %-14s %-14s\n" \
    "Conns" "CE req/s" "APISix req/s" "KrakenD req/s" "Kong req/s" "Tyk req/s"
  printf '%0.s─' {1..100}; echo ""

  for conns in "${RAMP_CONNS[@]}"; do
    local ceo="${RESULTS_DIR}/wrk_ramp_ce_${conns}.txt"
    local co="${RESULTS_DIR}/wrk_ramp_apisix_${conns}.txt"
    local ko="${RESULTS_DIR}/wrk_ramp_krakend_${conns}.txt"
    local kgo="${RESULTS_DIR}/wrk_ramp_kong_${conns}.txt"
    local to="${RESULTS_DIR}/wrk_ramp_tyk_${conns}.txt"

    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://ando-ce:9080/bench/plain > "${ceo}" 2>&1 || true
    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://apisix:8080/bench/plain > "${co}" 2>&1 || true
    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://krakend:8080/bench/plain > "${ko}" 2>&1 || true
    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://kong:8000/bench/plain > "${kgo}" 2>&1 || true
    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://tyk:8080/bench/plain > "${to}" 2>&1 || true

    local cer cep cr cp kr kp kgr kgp tr tp
    cer=$(extract_rps "${ceo}" || echo 0); cep=$(extract_p99 "${ceo}" || echo "N/A")
    cr=$(extract_rps  "${co}"  || echo 0); cp=$(extract_p99  "${co}"  || echo "N/A")
    kr=$(extract_rps  "${ko}"  || echo 0); kp=$(extract_p99  "${ko}"  || echo "N/A")
    kgr=$(extract_rps "${kgo}" || echo 0); kgp=$(extract_p99 "${kgo}" || echo "N/A")
    tr=$(extract_rps  "${to}"  || echo 0); tp=$(extract_p99  "${to}"  || echo "N/A")

    RAMP_CE_RPS+=("${cer}");      RAMP_CE_P99+=("${cep}")
    RAMP_APISIX_RPS+=("${cr}");   RAMP_APISIX_P99+=("${cp}")
    RAMP_KRAKEND_RPS+=("${kr}");  RAMP_KRAKEND_P99+=("${kp}")
    RAMP_KONG_RPS+=("${kgr}");    RAMP_KONG_P99+=("${kgp}")
    RAMP_TYK_RPS+=("${tr}");      RAMP_TYK_P99+=("${tp}")

    printf "%-8s %-14s %-14s %-14s %-14s %-14s\n" \
      "${conns}" "${cer}" "${cr}" "${kr}" "${kgr}" "${tr}"
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

  local ce_plain;  ce_plain=$(to_int  "${CE_PLAIN_RPS}")
  local c_plain;   c_plain=$(to_int   "${APISIX_PLAIN_RPS}")
  local k_plain;   k_plain=$(to_int   "${KRAKEND_PLAIN_RPS}")
  local kg_plain;  kg_plain=$(to_int  "${KONG_PLAIN_RPS}")
  local t_plain;   t_plain=$(to_int   "${TYK_PLAIN_RPS}")
  local ce_auth;   ce_auth=$(to_int   "${CE_AUTH_RPS}")
  local c_auth;    c_auth=$(to_int    "${APISIX_AUTH_RPS}")
  local k_auth;    k_auth=$(to_int    "${KRAKEND_AUTH_RPS}")
  local kg_auth;   kg_auth=$(to_int   "${KONG_AUTH_RPS}")
  local t_auth;    t_auth=$(to_int    "${TYK_AUTH_RPS}")
  local ce_stress; ce_stress=$(to_int "${CE_STRESS_RPS}")
  local c_stress;  c_stress=$(to_int  "${APISIX_STRESS_RPS}")
  local k_stress;  k_stress=$(to_int  "${KRAKEND_STRESS_RPS}")
  local kg_stress; kg_stress=$(to_int "${KONG_STRESS_RPS}")
  local t_stress;  t_stress=$(to_int  "${TYK_STRESS_RPS}")

  local ce_plain_ms;  ce_plain_ms=$(to_ms  "${CE_PLAIN_P99}")
  local c_plain_ms;   c_plain_ms=$(to_ms   "${APISIX_PLAIN_P99}")
  local k_plain_ms;   k_plain_ms=$(to_ms   "${KRAKEND_PLAIN_P99}")
  local kg_plain_ms;  kg_plain_ms=$(to_ms  "${KONG_PLAIN_P99}")
  local t_plain_ms;   t_plain_ms=$(to_ms   "${TYK_PLAIN_P99}")
  local ce_auth_ms;   ce_auth_ms=$(to_ms   "${CE_AUTH_P99}")
  local c_auth_ms;    c_auth_ms=$(to_ms    "${APISIX_AUTH_P99}")
  local k_auth_ms;    k_auth_ms=$(to_ms    "${KRAKEND_AUTH_P99}")
  local kg_auth_ms;   kg_auth_ms=$(to_ms   "${KONG_AUTH_P99}")
  local t_auth_ms;    t_auth_ms=$(to_ms    "${TYK_AUTH_P99}")
  local ce_stress_ms; ce_stress_ms=$(to_ms "${CE_STRESS_P99}")
  local c_stress_ms;  c_stress_ms=$(to_ms  "${APISIX_STRESS_P99}")
  local k_stress_ms;  k_stress_ms=$(to_ms  "${KRAKEND_STRESS_P99}")
  local kg_stress_ms; kg_stress_ms=$(to_ms "${KONG_STRESS_P99}")
  local t_stress_ms;  t_stress_ms=$(to_ms  "${TYK_STRESS_P99}")

  # Ramp CSV
  local ramp_ce_rps_csv="" ramp_apisix_rps_csv=""
  local ramp_krakend_rps_csv="" ramp_kong_rps_csv="" ramp_tyk_rps_csv=""
  local ramp_ce_p99_csv="" ramp_apisix_p99_csv=""
  local ramp_krakend_p99_csv="" ramp_kong_p99_csv="" ramp_tyk_p99_csv=""
  if [ ${#RAMP_CE_RPS[@]} -gt 0 ]; then
    local tmp=""
    for v in "${RAMP_CE_RPS[@]}";      do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done; ramp_ce_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_APISIX_RPS[@]}";  do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done; ramp_apisix_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_KRAKEND_RPS[@]}"; do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done; ramp_krakend_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_KONG_RPS[@]}";    do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done; ramp_kong_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_TYK_RPS[@]}";     do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done; ramp_tyk_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_CE_P99[@]}";      do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done; ramp_ce_p99_csv="${tmp}"; tmp=""
    for v in "${RAMP_APISIX_P99[@]}";  do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done; ramp_apisix_p99_csv="${tmp}"; tmp=""
    for v in "${RAMP_KRAKEND_P99[@]}"; do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done; ramp_krakend_p99_csv="${tmp}"; tmp=""
    for v in "${RAMP_KONG_P99[@]}";    do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done; ramp_kong_p99_csv="${tmp}"; tmp=""
    for v in "${RAMP_TYK_P99[@]}";     do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done; ramp_tyk_p99_csv="${tmp}"
  fi

  winner_of() {
    # usage: winner_of label1 val1 label2 val2 ...
    local max=0 winner=""
    while [[ $# -ge 2 ]]; do
      local label="$1" val="$2"; shift 2
      local vi; vi=$(echo "${val}" | awk '{printf "%d",$1*1000}' 2>/dev/null || echo 0)
      if (( vi > max )); then max=${vi}; winner="**${label}**"; fi
    done
    echo "${winner:-tie}"
  }

  cat > "${REPORT_FILE}" <<EOF
# Ando CE vs APISIX vs KrakenD vs Kong vs Tyk — Benchmark Report

**Date**        : ${ts}
**Host**        : ${cpu}
**Duration**    : ${DURATION} per scenario
**Threads**     : ${THREADS}
**Connections** : ${CONNECTIONS}  (stress: ${STRESS_CONNECTIONS})
**Run folder**  : \`$(basename "${RESULTS_DIR}")\`

---

## Throughput — Requests per Second (higher is better)

> Bar order: Ando CE | APISIX | KrakenD | Kong | Tyk

\`\`\`mermaid
xychart-beta
    title "Throughput — Requests per Second"
    x-axis ["Plain Proxy", "Key-Auth", "Stress (${STRESS_CONNECTIONS}c)"]
    y-axis "req/s"
    bar [${ce_plain}, ${ce_auth}, ${ce_stress}]
    bar [${c_plain},  ${c_auth},  ${c_stress}]
    bar [${k_plain},  ${k_auth},  ${k_stress}]
    bar [${kg_plain}, ${kg_auth}, ${kg_stress}]
    bar [${t_plain},  ${t_auth},  ${t_stress}]
\`\`\`

## p99 Latency — ms (lower is better)

> Bar order: Ando CE | APISIX | KrakenD | Kong | Tyk

\`\`\`mermaid
xychart-beta
    title "p99 Latency (ms)"
    x-axis ["Plain Proxy", "Key-Auth", "Stress (${STRESS_CONNECTIONS}c)"]
    y-axis "latency ms"
    bar [${ce_plain_ms}, ${ce_auth_ms}, ${ce_stress_ms}]
    bar [${c_plain_ms},  ${c_auth_ms},  ${c_stress_ms}]
    bar [${k_plain_ms},  ${k_auth_ms},  ${k_stress_ms}]
    bar [${kg_plain_ms}, ${kg_auth_ms}, ${kg_stress_ms}]
    bar [${t_plain_ms},  ${t_auth_ms},  ${t_stress_ms}]
\`\`\`

---

## Five-Way Comparison

| Gateway  | Plain req/s | Plain p99 | Auth req/s | Auth p99 | Stress req/s | Stress p99 | Plain Winner |
|----------|------------|-----------|-----------|----------|-------------|-----------|----|
| Ando CE  | ${CE_PLAIN_RPS:-N/A} | ${CE_PLAIN_P99:-N/A} | ${CE_AUTH_RPS:-N/A} | ${CE_AUTH_P99:-N/A} | ${CE_STRESS_RPS:-N/A} | ${CE_STRESS_P99:-N/A} | |
| APISIX   | ${APISIX_PLAIN_RPS:-N/A} | ${APISIX_PLAIN_P99:-N/A} | ${APISIX_AUTH_RPS:-N/A} | ${APISIX_AUTH_P99:-N/A} | ${APISIX_STRESS_RPS:-N/A} | ${APISIX_STRESS_P99:-N/A} | |
| KrakenD  | ${KRAKEND_PLAIN_RPS:-N/A} | ${KRAKEND_PLAIN_P99:-N/A} | ${KRAKEND_AUTH_RPS:-N/A} | ${KRAKEND_AUTH_P99:-N/A} | ${KRAKEND_STRESS_RPS:-N/A} | ${KRAKEND_STRESS_P99:-N/A} | |
| Kong     | ${KONG_PLAIN_RPS:-N/A} | ${KONG_PLAIN_P99:-N/A} | ${KONG_AUTH_RPS:-N/A} | ${KONG_AUTH_P99:-N/A} | ${KONG_STRESS_RPS:-N/A} | ${KONG_STRESS_P99:-N/A} | |
| Tyk      | ${TYK_PLAIN_RPS:-N/A} | ${TYK_PLAIN_P99:-N/A} | ${TYK_AUTH_RPS:-N/A} | ${TYK_AUTH_P99:-N/A} | ${TYK_STRESS_RPS:-N/A} | ${TYK_STRESS_P99:-N/A} | |
| **Winner** | $(winner_of "Ando CE" "${CE_PLAIN_RPS:-0}" "APISIX" "${APISIX_PLAIN_RPS:-0}" "KrakenD" "${KRAKEND_PLAIN_RPS:-0}" "Kong" "${KONG_PLAIN_RPS:-0}" "Tyk" "${TYK_PLAIN_RPS:-0}") | | $(winner_of "Ando CE" "${CE_AUTH_RPS:-0}" "APISIX" "${APISIX_AUTH_RPS:-0}" "KrakenD" "${KRAKEND_AUTH_RPS:-0}" "Kong" "${KONG_AUTH_RPS:-0}" "Tyk" "${TYK_AUTH_RPS:-0}") | | $(winner_of "Ando CE" "${CE_STRESS_RPS:-0}" "APISIX" "${APISIX_STRESS_RPS:-0}" "KrakenD" "${KRAKEND_STRESS_RPS:-0}" "Kong" "${KONG_STRESS_RPS:-0}" "Tyk" "${TYK_STRESS_RPS:-0}") | |

---

## Summary

| Scenario | req/s | p99 |
|---|---|---|
| Baseline (echo, no proxy) | ${BASELINE_RPS:-N/A} | ${BASELINE_P99:-N/A} |
| Ando CE plain proxy       | ${CE_PLAIN_RPS:-N/A} | ${CE_PLAIN_P99:-N/A} |
| APISIX plain proxy        | ${APISIX_PLAIN_RPS:-N/A} | ${APISIX_PLAIN_P99:-N/A} |
| KrakenD plain proxy       | ${KRAKEND_PLAIN_RPS:-N/A} | ${KRAKEND_PLAIN_P99:-N/A} |
| Kong plain proxy          | ${KONG_PLAIN_RPS:-N/A} | ${KONG_PLAIN_P99:-N/A} |
| Tyk plain proxy           | ${TYK_PLAIN_RPS:-N/A} | ${TYK_PLAIN_P99:-N/A} |
| Ando CE key-auth          | ${CE_AUTH_RPS:-N/A} | ${CE_AUTH_P99:-N/A} |
| APISIX key-auth           | ${APISIX_AUTH_RPS:-N/A} | ${APISIX_AUTH_P99:-N/A} |
| KrakenD key-auth (CEL)    | ${KRAKEND_AUTH_RPS:-N/A} | ${KRAKEND_AUTH_P99:-N/A} |
| Kong key-auth             | ${KONG_AUTH_RPS:-N/A} | ${KONG_AUTH_P99:-N/A} |
| Tyk key-auth              | ${TYK_AUTH_RPS:-N/A} | ${TYK_AUTH_P99:-N/A} |
| Ando CE stress (${STRESS_CONNECTIONS}c) | ${CE_STRESS_RPS:-N/A} | ${CE_STRESS_P99:-N/A} |
| APISIX stress (${STRESS_CONNECTIONS}c)  | ${APISIX_STRESS_RPS:-N/A} | ${APISIX_STRESS_P99:-N/A} |
| KrakenD stress (${STRESS_CONNECTIONS}c) | ${KRAKEND_STRESS_RPS:-N/A} | ${KRAKEND_STRESS_P99:-N/A} |
| Kong stress (${STRESS_CONNECTIONS}c)    | ${KONG_STRESS_RPS:-N/A} | ${KONG_STRESS_P99:-N/A} |
| Tyk stress (${STRESS_CONNECTIONS}c)     | ${TYK_STRESS_RPS:-N/A} | ${TYK_STRESS_P99:-N/A} |

EOF

  # Ramp section
  if [ -n "${ramp_ce_rps_csv}" ]; then
    cat >> "${REPORT_FILE}" <<EOF
---

## Scenario 4 — Concurrency Ramp (10 → 1000 connections)

> Line order: Ando CE | APISIX | KrakenD | Kong | Tyk

\`\`\`mermaid
xychart-beta
    title "Concurrency Ramp — Requests per Second"
    x-axis ["10c", "50c", "100c", "250c", "500c", "1000c"]
    y-axis "req/s"
    line [${ramp_ce_rps_csv}]
    line [${ramp_apisix_rps_csv}]
    line [${ramp_krakend_rps_csv}]
    line [${ramp_kong_rps_csv}]
    line [${ramp_tyk_rps_csv}]
\`\`\`

\`\`\`mermaid
xychart-beta
    title "Concurrency Ramp — p99 Latency (ms, lower is better)"
    x-axis ["10c", "50c", "100c", "250c", "500c", "1000c"]
    y-axis "latency ms"
    line [${ramp_ce_p99_csv}]
    line [${ramp_apisix_p99_csv}]
    line [${ramp_krakend_p99_csv}]
    line [${ramp_kong_p99_csv}]
    line [${ramp_tyk_p99_csv}]
\`\`\`

### Ramp Throughput (req/s)

| Conns | Ando CE | APISIX | KrakenD | Kong | Tyk |
|-------|---------|--------|---------|------|-----|
EOF
    local RAMP_CONNS=(10 50 100 250 500 1000)
    for i in "${!RAMP_CONNS[@]}"; do
      printf "| %s | %s | %s | %s | %s | %s |\n" \
        "${RAMP_CONNS[$i]}" \
        "${RAMP_CE_RPS[$i]:-N/A}" \
        "${RAMP_APISIX_RPS[$i]:-N/A}" \
        "${RAMP_KRAKEND_RPS[$i]:-N/A}" \
        "${RAMP_KONG_RPS[$i]:-N/A}" \
        "${RAMP_TYK_RPS[$i]:-N/A}" \
        >> "${REPORT_FILE}"
    done
    cat >> "${REPORT_FILE}" <<EOF

### Ramp p99 Latency (ms)

| Conns | Ando CE | APISIX | KrakenD | Kong | Tyk |
|-------|---------|--------|---------|------|-----|
EOF
    for i in "${!RAMP_CONNS[@]}"; do
      printf "| %s | %s | %s | %s | %s | %s |\n" \
        "${RAMP_CONNS[$i]}" \
        "${RAMP_CE_P99[$i]:-N/A}" \
        "${RAMP_APISIX_P99[$i]:-N/A}" \
        "${RAMP_KRAKEND_P99[$i]:-N/A}" \
        "${RAMP_KONG_P99[$i]:-N/A}" \
        "${RAMP_TYK_P99[$i]:-N/A}" \
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

### Plain Proxy — Ando CE
\`\`\`
$(cat "${RESULTS_DIR}/wrk_ce_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Plain Proxy — APISIX
\`\`\`
$(cat "${RESULTS_DIR}/wrk_apisix_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Key-Auth — Ando CE
\`\`\`
$(cat "${RESULTS_DIR}/wrk_ce_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Key-Auth — APISIX
\`\`\`
$(cat "${RESULTS_DIR}/wrk_apisix_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Stress Test — Ando CE
\`\`\`
$(cat "${RESULTS_DIR}/wrk_ce_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Stress Test — APISIX
\`\`\`
$(cat "${RESULTS_DIR}/wrk_apisix_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Plain Proxy — KrakenD
\`\`\`
$(cat "${RESULTS_DIR}/wrk_krakend_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Key-Auth — KrakenD
\`\`\`
$(cat "${RESULTS_DIR}/wrk_krakend_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Stress Test — KrakenD
\`\`\`
$(cat "${RESULTS_DIR}/wrk_krakend_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Plain Proxy — Kong
\`\`\`
$(cat "${RESULTS_DIR}/wrk_kong_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Key-Auth — Kong
\`\`\`
$(cat "${RESULTS_DIR}/wrk_kong_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Stress Test — Kong
\`\`\`
$(cat "${RESULTS_DIR}/wrk_kong_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Plain Proxy — Tyk
\`\`\`
$(cat "${RESULTS_DIR}/wrk_tyk_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Key-Auth — Tyk
\`\`\`
$(cat "${RESULTS_DIR}/wrk_tyk_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Stress Test — Tyk
\`\`\`
$(cat "${RESULTS_DIR}/wrk_tyk_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`
EOF

  ok "Report: ${REPORT_FILE}"
}

# ============================================================
# Entry point
# ============================================================
echo -e "\n${BOLD}${CYAN}"
echo "  ╔═════════════════════════════════════════════════════════╗"
echo "  ║  Ando CE vs APISIX vs KrakenD vs Kong vs Tyk             ║"
echo "  ╚═════════════════════════════════════════════════════════╝"
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
