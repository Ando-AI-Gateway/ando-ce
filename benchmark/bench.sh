#!/usr/bin/env bash
# ============================================================
# bench.sh — Ando vs APISIX Benchmark
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

# ---- Colors ----
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
  header "Starting services (first build may take a few minutes)"
  docker compose -f "${SCRIPT_DIR}/docker-compose.yml" up -d --build --wait
  ok "All services healthy"
}

# ── Docker helpers ────────────────────────────────────────────
dcurl() {
  docker run --rm --network "${BENCH_NET}" curlimages/curl:latest -sf "$@"
}



# ============================================================
# Main
# ============================================================
echo -e "\n${BOLD}${CYAN}"
echo "  ╔════════════════════════════════════════════╗"
echo "  ║   Ando vs APISIX Benchmark · Docker    ║"
  echo "  ╚══════════════════════════════════════════╝"
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

# ── Setup Ando routes ─────────────────────────────────────────
setup_routes() {
  header "Configuring Ando + APISIX routes"

  dcurl -X PUT "http://ando:9180/apisix/admin/upstreams/bench-echo" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-echo","type":"roundrobin","nodes":{"echo:3000":1}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "Upstream: bench-echo → echo:3000"

  dcurl -X PUT "http://ando:9180/apisix/admin/routes/bench-plain" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-plain","uri":"/bench/plain","methods":["GET"],"upstream_id":"bench-echo","plugins":{}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "Route: /bench/plain (no auth)"

  dcurl -X PUT "http://ando:9180/apisix/admin/consumers/bench-user" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"bench-user\",\"plugins\":{\"key-auth\":{\"key\":\"${API_KEY}\"}}}" \
    | grep -o '"username":"[^"]*"' || true

  dcurl -X PUT "http://ando:9180/apisix/admin/routes/bench-auth" \
    -H "Content-Type: application/json" \
    -d '{"id":"bench-auth","uri":"/bench/auth","methods":["GET"],"upstream_id":"bench-echo","plugins":{"key-auth":{}}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "Route: /bench/auth (key-auth)"

  # ── APISIX routes ──────────────────────────────────────────
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
  ok "APISIX upstream: bench-echo → echo:3000"

  dcurl -X PUT "http://apisix:9180/apisix/admin/routes/1" \
    -H "X-API-KEY: bench-admin-key-00000000000000" \
    -H "Content-Type: application/json" \
    -d '{"id":"1","uri":"/bench/plain","methods":["GET"],"upstream_id":"1","plugins":{}}' \
    | grep -o '"id":"[^"]*"' || true
  ok "APISIX route: /bench/plain (no auth)"

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
  ok "APISIX route: /bench/auth (key-auth)"
}

# ============================================================
# Benchmark scenarios
# ============================================================

# ── Warm-up ───────────────────────────────────────────────────
warmup() {
  local url="$1" name="$2"
  info "Warming up ${name} (10s)..."
  docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
    -t 4 -c 50 -d 10s "${url}" || warn "Warmup failed (continuing anyway)"
}

# ── Run wrk in Docker ─────────────────────────────────────────
# Outputs to ${RESULTS_DIR}/wrk_${label}.txt  (folder already has timestamp)
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
ANDO_PLAIN_RPS="" ANDO_PLAIN_P99="" APISIX_PLAIN_RPS="" APISIX_PLAIN_P99=""
ANDO_AUTH_RPS=""  ANDO_AUTH_P99=""  APISIX_AUTH_RPS=""  APISIX_AUTH_P99=""
ANDO_STRESS_RPS="" ANDO_STRESS_P99="" APISIX_STRESS_RPS="" APISIX_STRESS_P99=""
RAMP_ANDO_RPS=()   RAMP_ANDO_P99=()
RAMP_APISIX_RPS=() RAMP_APISIX_P99=()

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
  warmup "http://ando:9080/bench/plain" "Ando"
  run_wrk "http://ando:9080/bench/plain" "ando_plain"
  ANDO_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_ando_plain.txt")
  ANDO_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_ando_plain.txt")
  ok "Ando  plain: ${ANDO_PLAIN_RPS:-?} req/s  p99 ${ANDO_PLAIN_P99:-?}"

  warmup "http://apisix:8080/bench/plain" "APISIX"
  run_wrk "http://apisix:8080/bench/plain" "apisix_plain"
  APISIX_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_plain.txt")
  APISIX_PLAIN_P99=$(extract_p99  "${RESULTS_DIR}/wrk_apisix_plain.txt")
  ok "APISIX plain: ${APISIX_PLAIN_RPS:-?} req/s  p99 ${APISIX_PLAIN_P99:-?}"
}

bench_auth() {
  header "Scenario 2 — Key-Auth Plugin (${CONNECTIONS} conns, ${DURATION})"
  warmup "http://ando:9080/bench/auth" "Ando+auth"
  run_wrk "http://ando:9080/bench/auth" "ando_auth" "apikey: ${API_KEY}"
  ANDO_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_ando_auth.txt")
  ANDO_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_ando_auth.txt")
  ok "Ando  auth: ${ANDO_AUTH_RPS:-?} req/s  p99 ${ANDO_AUTH_P99:-?}"

  warmup "http://apisix:8080/bench/auth" "APISIX+auth"
  run_wrk "http://apisix:8080/bench/auth" "apisix_auth" "apikey: ${API_KEY}"
  APISIX_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_auth.txt")
  APISIX_AUTH_P99=$(extract_p99  "${RESULTS_DIR}/wrk_apisix_auth.txt")
  ok "APISIX auth: ${APISIX_AUTH_RPS:-?} req/s  p99 ${APISIX_AUTH_P99:-?}"
}

bench_stress() {
  header "Scenario 3 — Stress Test (${STRESS_CONNECTIONS} conns, ${DURATION})"
  warn "High concurrency — some errors at saturation are expected."
  warmup "http://ando:9080/bench/plain" "Ando"
  run_wrk "http://ando:9080/bench/plain" "ando_stress" "" "${STRESS_CONNECTIONS}"
  ANDO_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_ando_stress.txt")
  ANDO_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_ando_stress.txt")
  ok "Ando  stress: ${ANDO_STRESS_RPS:-?} req/s  p99 ${ANDO_STRESS_P99:-?}"

  warmup "http://apisix:8080/bench/plain" "APISIX"
  run_wrk "http://apisix:8080/bench/plain" "apisix_stress" "" "${STRESS_CONNECTIONS}"
  APISIX_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_stress.txt")
  APISIX_STRESS_P99=$(extract_p99  "${RESULTS_DIR}/wrk_apisix_stress.txt")
  ok "APISIX stress: ${APISIX_STRESS_RPS:-?} req/s  p99 ${APISIX_STRESS_P99:-?}"
}

bench_ramp() {
  header "Scenario 4 — Concurrency Ramp (10 → 1000)"
  local RAMP_CONNS=(10 50 100 250 500 1000)
  local old_dur="${DURATION}"; DURATION="15s"

  printf "\n%-8s %-18s %-14s %-18s %-14s\n" "Conns" "Ando req/s" "Ando p99" "APISIX req/s" "APISIX p99"
  printf '%0.s─' {1..74}; echo ""

  for conns in "${RAMP_CONNS[@]}"; do
    local ao="${RESULTS_DIR}/wrk_ramp_ando_${conns}.txt"
    local co="${RESULTS_DIR}/wrk_ramp_apisix_${conns}.txt"
    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://ando:9080/bench/plain > "${ao}" 2>&1 || true
    docker run --rm --network "${BENCH_NET}" "${WRK_IMAGE}" \
      -t "${THREADS}" -c "${conns}" -d "${DURATION}" --latency \
      http://apisix:8080/bench/plain > "${co}" 2>&1 || true
    local ar ap cr cp
    ar=$(extract_rps "${ao}" || echo 0)
    ap=$(extract_p99  "${ao}" || echo "N/A")
    cr=$(extract_rps "${co}" || echo 0)
    cp=$(extract_p99  "${co}" || echo "N/A")
    RAMP_ANDO_RPS+=("${ar}")
    RAMP_ANDO_P99+=("${ap}")
    RAMP_APISIX_RPS+=("${cr}")
    RAMP_APISIX_P99+=("${cp}")
    printf "%-8s %-18s %-14s %-18s %-14s\n" \
      "${conns}" "${ar}" "${ap}" "${cr}" "${cp}"
  done
  DURATION="${old_dur}"
}

# ── Report ────────────────────────────────────────────────────
write_report() {
  local ts cpu
  ts=$(date "+%Y-%m-%d %H:%M:%S %Z")
  cpu=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || \
        grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo "unknown")

  # ── Helper: strip units, convert to integer/float ───────────
  to_int() { echo "${1:-0}" | sed 's/[^0-9.]//g' | awk '{printf "%d",$1+0}'; }
  to_ms()  { echo "${1:-0}" | sed 's/ms//' | awk '{printf "%.2f",$1+0}'; }

  # ── Pre-compute chart values (integers for Mermaid) ─────────
  local a_plain;  a_plain=$(to_int  "${ANDO_PLAIN_RPS}")
  local c_plain;  c_plain=$(to_int  "${APISIX_PLAIN_RPS}")
  local a_auth;   a_auth=$(to_int   "${ANDO_AUTH_RPS}")
  local c_auth;   c_auth=$(to_int   "${APISIX_AUTH_RPS}")
  local a_stress; a_stress=$(to_int "${ANDO_STRESS_RPS}")
  local c_stress; c_stress=$(to_int "${APISIX_STRESS_RPS}")

  local a_plain_ms;  a_plain_ms=$(to_ms  "${ANDO_PLAIN_P99}")
  local c_plain_ms;  c_plain_ms=$(to_ms  "${APISIX_PLAIN_P99}")
  local a_auth_ms;   a_auth_ms=$(to_ms   "${ANDO_AUTH_P99}")
  local c_auth_ms;   c_auth_ms=$(to_ms   "${APISIX_AUTH_P99}")
  local a_stress_ms; a_stress_ms=$(to_ms "${ANDO_STRESS_P99}")
  local c_stress_ms; c_stress_ms=$(to_ms "${APISIX_STRESS_P99}")

  # ── Ramp CSV values for line charts ─────────────────────────
  local ramp_ando_rps_csv="" ramp_apisix_rps_csv=""
  local ramp_ando_p99_csv="" ramp_apisix_p99_csv=""
  if [ ${#RAMP_ANDO_RPS[@]} -gt 0 ]; then
    local tmp=""
    for v in "${RAMP_ANDO_RPS[@]}";   do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done
    ramp_ando_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_APISIX_RPS[@]}"; do tmp="${tmp:+${tmp}, }$(to_int "${v}")"; done
    ramp_apisix_rps_csv="${tmp}"; tmp=""
    for v in "${RAMP_ANDO_P99[@]}";   do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done
    ramp_ando_p99_csv="${tmp}"; tmp=""
    for v in "${RAMP_APISIX_P99[@]}"; do tmp="${tmp:+${tmp}, }$(to_ms  "${v}")"; done
    ramp_apisix_p99_csv="${tmp}"
  fi

  local ando_eff=""
  if [[ "${BASELINE_RPS:-}" =~ ^[0-9] ]] && [[ "${ANDO_PLAIN_RPS:-}" =~ ^[0-9] ]]; then
    ando_eff=$(awk "BEGIN{printf \"%.1f\",($ANDO_PLAIN_RPS/$BASELINE_RPS)*100}")%
  fi

  winner() {
    local a="${1:-0}" b="${2:-0}"
    local ai bi
    ai=$(echo "${a}" | awk '{printf "%d",$1*1000}' 2>/dev/null || echo 0)
    bi=$(echo "${b}" | awk '{printf "%d",$1*1000}' 2>/dev/null || echo 0)
    [[ ! "${a}" =~ ^[0-9] ]] || [[ ! "${b}" =~ ^[0-9] ]] && { echo "N/A"; return; }
    [ "${ai}" -gt "${bi}" ] && echo "**Ando**"  && return
    [ "${bi}" -gt "${ai}" ] && echo "**APISIX**" && return
    echo "tie"
  }

  # ── Part 1: Header + throughput & latency charts ─────────────
  cat > "${REPORT_FILE}" <<EOF
# Ando vs APISIX Benchmark Report

**Date**        : ${ts}
**Host**        : ${cpu}
**Duration**    : ${DURATION} per scenario
**Threads**     : ${THREADS}
**Connections** : ${CONNECTIONS}  (stress: ${STRESS_CONNECTIONS})
**Run folder**  : \`$(basename "${RESULTS_DIR}")\`

---

## Throughput Comparison (req/s)

> **Bar 1 = Ando** &nbsp;|&nbsp; **Bar 2 = APISIX** &nbsp;—&nbsp; Higher is better

\`\`\`mermaid
xychart-beta
    title "Throughput — Requests per Second"
    x-axis ["Plain Proxy", "Key-Auth", "Stress (${STRESS_CONNECTIONS}c)"]
    y-axis "req/s"
    bar [${a_plain}, ${a_auth}, ${a_stress}]
    bar [${c_plain}, ${c_auth}, ${c_stress}]
\`\`\`

## p99 Latency Comparison (ms)

> **Bar 1 = Ando** &nbsp;|&nbsp; **Bar 2 = APISIX** &nbsp;—&nbsp; Lower is better

\`\`\`mermaid
xychart-beta
    title "p99 Latency (ms) — Lower is better"
    x-axis ["Plain Proxy", "Key-Auth", "Stress (${STRESS_CONNECTIONS}c)"]
    y-axis "latency ms"
    bar [${a_plain_ms}, ${a_auth_ms}, ${a_stress_ms}]
    bar [${c_plain_ms}, ${c_auth_ms}, ${c_stress_ms}]
\`\`\`

---

## Ando vs APISIX Comparison

| Scenario | Ando req/s | Ando p99 | APISIX req/s | APISIX p99 | Winner |
|---|---|---|---|---|---|
| Plain Proxy (${CONNECTIONS}c) | ${ANDO_PLAIN_RPS:-N/A} | ${ANDO_PLAIN_P99:-N/A} | ${APISIX_PLAIN_RPS:-N/A} | ${APISIX_PLAIN_P99:-N/A} | $(winner "${ANDO_PLAIN_RPS:-0}" "${APISIX_PLAIN_RPS:-0}") |
| Key-Auth (${CONNECTIONS}c) | ${ANDO_AUTH_RPS:-N/A} | ${ANDO_AUTH_P99:-N/A} | ${APISIX_AUTH_RPS:-N/A} | ${APISIX_AUTH_P99:-N/A} | $(winner "${ANDO_AUTH_RPS:-0}" "${APISIX_AUTH_RPS:-0}") |
| Stress Test (${STRESS_CONNECTIONS}c) | ${ANDO_STRESS_RPS:-N/A} | ${ANDO_STRESS_P99:-N/A} | ${APISIX_STRESS_RPS:-N/A} | ${APISIX_STRESS_P99:-N/A} | $(winner "${ANDO_STRESS_RPS:-0}" "${APISIX_STRESS_RPS:-0}") |

---

## Summary

| Scenario | req/s | p99 |
|---|---|---|
| Baseline (echo, no proxy) | ${BASELINE_RPS:-N/A} | ${BASELINE_P99:-N/A} |
| Ando plain proxy | ${ANDO_PLAIN_RPS:-N/A} | ${ANDO_PLAIN_P99:-N/A} |
| Ando key-auth plugin | ${ANDO_AUTH_RPS:-N/A} | ${ANDO_AUTH_P99:-N/A} |
| Ando stress (${STRESS_CONNECTIONS}c) | ${ANDO_STRESS_RPS:-N/A} | ${ANDO_STRESS_P99:-N/A} |
| APISIX plain proxy | ${APISIX_PLAIN_RPS:-N/A} | ${APISIX_PLAIN_P99:-N/A} |
| APISIX key-auth plugin | ${APISIX_AUTH_RPS:-N/A} | ${APISIX_AUTH_P99:-N/A} |
| APISIX stress (${STRESS_CONNECTIONS}c) | ${APISIX_STRESS_RPS:-N/A} | ${APISIX_STRESS_P99:-N/A} |

${ando_eff:+> **Ando proxy efficiency**: ${ando_eff} of raw backend throughput}

EOF

  # ── Part 2: Ramp charts (only when ramp was run) ─────────────
  if [ -n "${ramp_ando_rps_csv}" ]; then
    cat >> "${REPORT_FILE}" <<EOF
---

## Scenario 4 — Concurrency Ramp (10 → 1000 connections)

> **Line 1 = Ando** &nbsp;|&nbsp; **Line 2 = APISIX**

\`\`\`mermaid
xychart-beta
    title "Concurrency Ramp — Requests per Second"
    x-axis ["10c", "50c", "100c", "250c", "500c", "1000c"]
    y-axis "req/s"
    line [${ramp_ando_rps_csv}]
    line [${ramp_apisix_rps_csv}]
\`\`\`

\`\`\`mermaid
xychart-beta
    title "Concurrency Ramp — p99 Latency (ms, lower is better)"
    x-axis ["10c", "50c", "100c", "250c", "500c", "1000c"]
    y-axis "latency ms"
    line [${ramp_ando_p99_csv}]
    line [${ramp_apisix_p99_csv}]
\`\`\`

### Ramp Data Table

| Conns | Ando req/s | Ando p99 | APISIX req/s | APISIX p99 |
|---|---|---|---|---|
EOF
    local RAMP_CONNS=(10 50 100 250 500 1000)
    for i in "${!RAMP_CONNS[@]}"; do
      printf "| %s | %s | %s | %s | %s |\n" \
        "${RAMP_CONNS[$i]}" \
        "${RAMP_ANDO_RPS[$i]:-N/A}" "${RAMP_ANDO_P99[$i]:-N/A}" \
        "${RAMP_APISIX_RPS[$i]:-N/A}" "${RAMP_APISIX_P99[$i]:-N/A}" \
        >> "${REPORT_FILE}"
    done
    echo "" >> "${REPORT_FILE}"
  fi

  # ── Part 3: Raw wrk outputs ──────────────────────────────────
  cat >> "${REPORT_FILE}" <<EOF
---

## Raw wrk Outputs

### Scenario 0 — Baseline
\`\`\`
$(cat "${RESULTS_DIR}/wrk_baseline.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Scenario 1 — Plain Proxy (Ando)
\`\`\`
$(cat "${RESULTS_DIR}/wrk_ando_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Scenario 1 — Plain Proxy (APISIX)
\`\`\`
$(cat "${RESULTS_DIR}/wrk_apisix_plain.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Scenario 2 — Key-Auth Plugin (Ando)
\`\`\`
$(cat "${RESULTS_DIR}/wrk_ando_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Scenario 2 — Key-Auth Plugin (APISIX)
\`\`\`
$(cat "${RESULTS_DIR}/wrk_apisix_auth.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Scenario 3 — Stress Test (Ando)
\`\`\`
$(cat "${RESULTS_DIR}/wrk_ando_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`

### Scenario 3 — Stress Test (APISIX)
\`\`\`
$(cat "${RESULTS_DIR}/wrk_apisix_stress.txt" 2>/dev/null || echo "N/A")
\`\`\`

## Ando Log (last 30 lines)
\`\`\`
$(docker compose -f "${SCRIPT_DIR}/docker-compose.yml" logs --tail 30 ando 2>/dev/null || echo "N/A")
\`\`\`
EOF

  ok "Report: ${REPORT_FILE}"
}

# ============================================================
# Run
# ============================================================
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
