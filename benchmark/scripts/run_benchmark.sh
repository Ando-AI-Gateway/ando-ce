#!/usr/bin/env bash
# ============================================================
# run_benchmark.sh — Ando vs APISIX Benchmark Runner
# ============================================================
#
# Prerequisites (install once):
#   brew install wrk          # or: brew install wrk2
#   brew install k6
#   brew install hey
#
# Usage:
#   ./scripts/run_benchmark.sh [scenario]
#
# Scenarios:
#   all      (default) — run all scenarios
#   plain    — plain proxy, no plugins
#   auth     — key-auth plugin enabled
#   ramp     — connection ramp-up test
# ============================================================

set -euo pipefail

# ── Config ────────────────────────────────────────────────────
ANDO_URL="${ANDO_URL:-http://localhost:9080}"
APISIX_URL="${APISIX_URL:-http://localhost:8080}"
RESULTS_DIR="${RESULTS_DIR:-$(dirname "$0")/../results}"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
REPORT_FILE="${RESULTS_DIR}/report_${TIMESTAMP}.md"
SCENARIO="${1:-all}"

# Benchmark parameters
DURATION="${BENCH_DURATION:-60s}"
CONNECTIONS="${BENCH_CONNECTIONS:-500}"
THREADS="${BENCH_THREADS:-$(( $(sysctl -n hw.logicalcpu 2>/dev/null || nproc 2>/dev/null || echo 4) * 2 ))}"
WRM2_RATE="${BENCH_WRK2_RATE:-50000}"   # constant-rate target for wrk2 (rps)
STRESS_CONNECTIONS="${BENCH_STRESS_CONNECTIONS:-1000}"  # for stress scenario
API_KEY="bench-secret-key"

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

info()    { echo -e "${BLUE}[INFO]${NC}  $*"; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }
header()  { echo -e "\n${BOLD}${CYAN}════════════════════════════════════════${NC}"; echo -e "${BOLD}  $*${NC}"; echo -e "${BOLD}${CYAN}════════════════════════════════════════${NC}"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }

# ── Tool detection ─────────────────────────────────────────────
detect_tools() {
  LOAD_TOOL=""
  if command -v wrk2 &>/dev/null; then
    LOAD_TOOL="wrk2"
  elif command -v wrk &>/dev/null; then
    LOAD_TOOL="wrk"
    warn "wrk2 not found; using wrk. Install wrk2 for accurate latency percentiles."
    warn "  brew install wrk2   (or: brew tap jabley/homebrew-wrk2 && brew install wrk2)"
  else
    warn "Neither wrk nor wrk2 found. Falling back to 'hey'."
    if ! command -v hey &>/dev/null; then
      echo -e "${RED}[ERR]${NC} No load tool found. Install one of:"; \
      echo "  brew install wrk"
      echo "  brew install hey"
      exit 1
    fi
    LOAD_TOOL="hey"
  fi
  info "Load tool: ${LOAD_TOOL}"
}

# ── Ensure results dir ────────────────────────────────────────
mkdir -p "${RESULTS_DIR}"

# ── Warm-up (5s pre-run to prime connection pools + JIT) ─────
warmup() {
  local url="$1" name="$2"
  info "Warming up ${name} (5s)..."
  if [ "$LOAD_TOOL" = "hey" ]; then
    hey -z 5s -c 50 -q 0 "${url}" > /dev/null 2>&1 || true
  elif command -v wrk &>/dev/null || command -v wrk2 &>/dev/null; then
    wrk -t 4 -c 50 -d 5s "${url}" > /dev/null 2>&1 || true
  fi
}

# ── Run wrk/wrk2 with configurable connection count ───────────
run_wrk() {
  local url="$1" name="$2" extra_args="${3:-}" conns="${4:-${CONNECTIONS}}"
  local out_file="${RESULTS_DIR}/wrk_${name//\//_}_${TIMESTAMP}.txt"

  # Convert extra_args (specifically headers) to array for safe shell expansion
  # Handles "-H 'name: value'" format
  local cmd_args=()
  if [[ "$extra_args" == -H* ]]; then
    # Strip leading -H and quotes
    local h_val=${extra_args#"-H "}
    h_val=${h_val#"'"}
    h_val=${h_val%"'"}
    cmd_args=("-H" "$h_val")
  elif [ -n "$extra_args" ]; then
    cmd_args=($extra_args)
  fi

  if [ "$LOAD_TOOL" = "wrk2" ]; then
    wrk2 -t "${THREADS}" -c "${conns}" -d "${DURATION}" \
      -R "${WRM2_RATE}" --latency "${cmd_args[@]}" "${url}" \
      2>&1 | tee "${out_file}"
  elif [ "$LOAD_TOOL" = "wrk" ]; then
    wrk -t "${THREADS}" -c "${conns}" -d "${DURATION}" \
      --latency "${cmd_args[@]}" "${url}" \
      2>&1 | tee "${out_file}"
  else
    # hey uses -H for headers but often different syntax
    hey -z "${DURATION}" -c "${conns}" -q 0 \
      "${cmd_args[@]}" "${url}" \
      2>&1 | tee "${out_file}"
  fi

  cat "${out_file}"
}

# ── Extract key metric (Requests/sec or RPS) ──────────────────
extract_rps() {
  local file="$1"
  if [ "$LOAD_TOOL" = "hey" ]; then
    grep "Requests/sec" "${file}" | awk '{print $2}' | tail -1
  else
    grep "Req/Sec\|Requests/sec" "${file}" | awk '{print $2}' | tail -1
  fi
}

# ── Scenario: Plain proxy ─────────────────────────────────────
bench_plain() {
  header "Scenario 1: Plain Proxy (no plugins) — ${CONNECTIONS} connections, ${THREADS} threads"

  info "→ Running against Ando..."
  warmup "${ANDO_URL}/bench/plain" "Ando"
  ANDO_PLAIN=$(run_wrk "${ANDO_URL}/bench/plain" "ando_plain" "" "${CONNECTIONS}")
  ANDO_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_ando_plain_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")

  echo ""
  info "→ Running against APISIX..."
  warmup "${APISIX_URL}/bench/plain" "APISIX"
  APISIX_PLAIN=$(run_wrk "${APISIX_URL}/bench/plain" "apisix_plain" "" "${CONNECTIONS}")
  APISIX_PLAIN_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_plain_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")
}

# ── Scenario: Key-auth plugin ─────────────────────────────────
bench_auth() {
  header "Scenario 2: Key-Auth Plugin — ${CONNECTIONS} connections"

  info "→ Running against Ando..."
  warmup "${ANDO_URL}/bench/auth" "Ando"
  ANDO_AUTH=$(run_wrk "${ANDO_URL}/bench/auth" "ando_auth" \
    "-H 'apikey: ${API_KEY}'" "${CONNECTIONS}")
  ANDO_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_ando_auth_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")

  echo ""
  info "→ Running against APISIX..."
  warmup "${APISIX_URL}/bench/auth" "APISIX"
  APISIX_AUTH=$(run_wrk "${APISIX_URL}/bench/auth" "apisix_auth" \
    "-H 'apikey: ${API_KEY}'" "${CONNECTIONS}")
  APISIX_AUTH_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_auth_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")
}

# ── Scenario: Connection ramp ─────────────────────────────────
bench_ramp() {
  header "Scenario 3: Concurrency Ramp (10 → 1000 connections)"

  local old_duration="$DURATION"
  DURATION="20s"   # shorter per step so total time is manageable
  local RAMP_CONNS=(10 50 100 250 500 1000)

  printf "%-14s %-20s %-20s\n" "Connections" "Ando (Req/s)" "APISIX (Req/s)"
  printf '%s\n' "------------------------------------------------------------"

  for conns in "${RAMP_CONNS[@]}"; do
    ando_rps=$(run_wrk "${ANDO_URL}/bench/plain" "ando_ramp_${conns}" "" "${conns}" \
      2>/dev/null | grep -E "Req/Sec|Requests/sec" | awk '{print $2}' | tail -1 || echo "N/A")
    apisix_rps=$(run_wrk "${APISIX_URL}/bench/plain" "apisix_ramp_${conns}" "" "${conns}" \
      2>/dev/null | grep -E "Req/Sec|Requests/sec" | awk '{print $2}' | tail -1 || echo "N/A")
    printf "%-14s %-20s %-20s\n" "${conns}" "${ando_rps}" "${apisix_rps}"
  done

  DURATION="$old_duration"
}

# ── Scenario: High-concurrency stress ─────────────────────────
bench_stress() {
  header "Scenario 4: Stress Test — ${STRESS_CONNECTIONS} connections, ${DURATION}"
  echo -e "  ${YELLOW}(Pushing both gateways to limits — errors expected at saturation)${NC}"
  echo ""

  info "→ Ando stress test..."
  warmup "${ANDO_URL}/bench/plain" "Ando"
  ANDO_STRESS=$(run_wrk "${ANDO_URL}/bench/plain" "ando_stress" "" "${STRESS_CONNECTIONS}")
  ANDO_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_ando_stress_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")

  echo ""
  info "→ APISIX stress test..."
  warmup "${APISIX_URL}/bench/plain" "APISIX"
  APISIX_STRESS=$(run_wrk "${APISIX_URL}/bench/plain" "apisix_stress" "" "${STRESS_CONNECTIONS}")
  APISIX_STRESS_RPS=$(extract_rps "${RESULTS_DIR}/wrk_apisix_stress_${TIMESTAMP}.txt" 2>/dev/null || echo "N/A")
}

# ── Markdown report ───────────────────────────────────────────
write_report() {
  local ts
  ts=$(date "+%Y-%m-%d %H:%M:%S %Z")

  # Determine winners
  winner_plain="🤝 Tie"
  winner_auth="🤝 Tie"
  winner_stress="🤝 Tie"
  if [[ "${ANDO_PLAIN_RPS:-0}" =~ ^[0-9] ]] && [[ "${APISIX_PLAIN_RPS:-0}" =~ ^[0-9] ]]; then
    ando_n=$(echo "${ANDO_PLAIN_RPS}" | tr -d 'k' | awk '{print $1+0}')
    apisix_n=$(echo "${APISIX_PLAIN_RPS}" | tr -d 'k' | awk '{print $1+0}')
    [[ $(echo "$ando_n > $apisix_n" | bc -l 2>/dev/null || echo 0) -eq 1 ]] && winner_plain="🏆 Ando" || winner_plain="🏆 APISIX"
  fi

cat > "${REPORT_FILE}" << EOF
# Ando vs APISIX Benchmark Report

**Date**: ${ts}  
**Load tool**: ${LOAD_TOOL}  
**Duration per scenario**: ${DURATION}  
**Connections**: ${CONNECTIONS} (stress: ${STRESS_CONNECTIONS})  
**Threads**: ${THREADS}  
**wrk2 target rate**: ${WRM2_RATE:-N/A} rps  

---

## Summary

| Scenario | Ando RPS | APISIX RPS | Winner |
|---|---|---|---|
| Plain proxy (${CONNECTIONS}c) | ${ANDO_PLAIN_RPS:-N/A} | ${APISIX_PLAIN_RPS:-N/A} | ${winner_plain} |
| Key-auth plugin (${CONNECTIONS}c) | ${ANDO_AUTH_RPS:-N/A} | ${APISIX_AUTH_RPS:-N/A} | ${winner_auth} |
| Stress (${STRESS_CONNECTIONS}c) | ${ANDO_STRESS_RPS:-N/A} | ${APISIX_STRESS_RPS:-N/A} | ${winner_stress} |

> Latency percentiles (p50/p95/p99) in raw output below.

---

## Scenario 1 — Plain Proxy

### Ando
\`\`\`
${ANDO_PLAIN:-}
\`\`\`

### APISIX
\`\`\`
${APISIX_PLAIN:-}
\`\`\`

## Scenario 2 — Key-Auth Plugin

### Ando
\`\`\`
${ANDO_AUTH:-}
\`\`\`

### APISIX
\`\`\`
${APISIX_AUTH:-}
\`\`\`

## Scenario 3 — Stress Test (${STRESS_CONNECTIONS} connections)

### Ando
\`\`\`
${ANDO_STRESS:-N/A}
\`\`\`

### APISIX
\`\`\`
${APISIX_STRESS:-N/A}
\`\`\`
EOF

  success "Report written to: ${REPORT_FILE}"
}

# ── Main ──────────────────────────────────────────────────────
main() {
  echo -e "\n${BOLD}${CYAN}"
  echo "  ╔═══════════════════════════════════════╗"
  echo "  ║   Ando vs APISIX Benchmark Suite      ║"
  echo "  ╚═══════════════════════════════════════╝"
  echo -e "${NC}"

  detect_tools
  info "Results directory: ${RESULTS_DIR}"
  info "Report will be written to: ${REPORT_FILE}"
  echo ""

  ANDO_PLAIN="" APISIX_PLAIN="" ANDO_AUTH="" APISIX_AUTH=""
  ANDO_PLAIN_RPS="" APISIX_PLAIN_RPS="" ANDO_AUTH_RPS="" APISIX_AUTH_RPS=""

  ANDO_STRESS="" APISIX_STRESS="" ANDO_STRESS_RPS="" APISIX_STRESS_RPS=""

  case "$SCENARIO" in
    all)
      bench_plain
      bench_auth
      bench_stress
      bench_ramp
      write_report
      ;;
    plain)
      bench_plain
      write_report
      ;;
    auth)
      bench_auth
      write_report
      ;;
    stress)
      bench_stress
      write_report
      ;;
    ramp)
      bench_ramp
      ;;
    *)
      echo "Unknown scenario: ${SCENARIO}"
      echo "Usage: $0 [all|plain|auth|stress|ramp]"
      exit 1
      ;;
  esac

  header "Done!"
  echo ""
  success "Open your report: open ${REPORT_FILE}"
}

main
