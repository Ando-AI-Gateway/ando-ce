#!/usr/bin/env bash
# ============================================================
# bench.sh — One-command Ando vs APISIX benchmark
# ============================================================
# Run from project root:
#   chmod +x benchmark/bench.sh
#   ./benchmark/bench.sh
# ============================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.bench.yml"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'
BOLD='\033[1m'; NC='\033[0m'

info()   { echo -e "${CYAN}[INFO]${NC}  $*"; }
ok()     { echo -e "${GREEN}[OK]${NC}    $*"; }
header() { echo -e "\n${BOLD}${CYAN}$*${NC}"; }

echo -e "${BOLD}${CYAN}"
echo "  ╔═══════════════════════════════════════╗"
echo "  ║   Ando vs APISIX — Quick Benchmark    ║"
echo "  ╚═══════════════════════════════════════╝"
echo -e "${NC}"

# ── Step 1: Start the stack ───────────────────────────────────
header "▶ Step 1: Starting benchmark stack..."
docker compose -f "${COMPOSE_FILE}" up -d --build
ok "Stack started"

# ── Step 2: Wait for health ───────────────────────────────────
header "▶ Step 2: Waiting for all services to be healthy..."
docker compose -f "${COMPOSE_FILE}" ps

info "Giving services 20s to fully start..."
sleep 20

# ── Step 3: Setup routes ──────────────────────────────────────
header "▶ Step 3: Configuring routes on both gateways..."
bash "${SCRIPT_DIR}/scripts/setup_routes.sh"
ok "Routes configured"

# ── Step 4: Run benchmark ─────────────────────────────────────
header "▶ Step 4: Running benchmarks..."
echo ""
echo -e "${YELLOW}Tip: Set BENCH_DURATION, BENCH_CONNECTIONS to tune the load.${NC}"
echo -e "${YELLOW}     e.g.  BENCH_DURATION=60s BENCH_CONNECTIONS=200 ./benchmark/bench.sh${NC}"
echo ""

bash "${SCRIPT_DIR}/scripts/run_benchmark.sh" "${1:-all}"

# ── Step 5: Done ──────────────────────────────────────────────
header "▶ Done!"
ok "Results saved in: benchmark/results/"
echo ""
echo "To stop the stack:  docker compose -f benchmark/docker-compose.bench.yml down"
echo "To run k6 instead:  ./benchmark/bench.sh k6"
echo ""
