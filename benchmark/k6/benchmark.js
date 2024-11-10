import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Rate, Counter } from 'k6/metrics';

// ============================================================
// k6 Heavy Concurrent Benchmark: Ando vs APISIX
// ============================================================
// Run locally (against Docker stack):
//   k6 run --env TARGET=ando  benchmark/k6/benchmark.js
//   k6 run --env TARGET=apisix benchmark/k6/benchmark.js
//
// Run inside Docker (from bench.sh):
//   docker compose -f benchmark/docker-compose.bench.yml \
//     run --rm bench k6 run --env TARGET=ando --env IN_DOCKER=1 /scripts/benchmark.js
//
// Env vars:
//   TARGET      ando | apisix (default: ando)
//   IN_DOCKER   1 = use container hostnames (default: localhost)
//   SCENARIO    quick | heavy | stress | spike | soak (default: heavy)
// ============================================================

const TARGET = __ENV.TARGET || 'ando';
const IN_DOCKER = __ENV.IN_DOCKER === '1';
const SCENARIO = __ENV.SCENARIO || 'heavy';

// Host resolution
const HOSTS = {
  docker: {
    ando: 'http://ando:9080',
    apisix: 'http://apisix:9080',
  },
  local: {
    ando: 'http://localhost:9080',
    apisix: 'http://localhost:8080',
  },
};

const BASE_URL = (IN_DOCKER ? HOSTS.docker : HOSTS.local)[TARGET];
const API_KEY = 'bench-secret-key';

// ── Custom metrics ────────────────────────────────────────────
const latencyPlain = new Trend('latency_plain_ms', true);
const latencyAuth = new Trend('latency_auth_ms', true);
const errorRate = new Rate('errors');
const reqCount = new Counter('requests_total');

// ── Scenario definitions ──────────────────────────────────────
const SCENARIOS = {
  // Quick smoke-test (CI / first check)
  quick: {
    plain_quick: {
      executor: 'constant-arrival-rate',
      rate: 500, timeUnit: '1s', duration: '15s',
      preAllocatedVUs: 50, maxVUs: 200,
      exec: 'plain',
    },
  },

  // Heavy sustained load — 2000 rps, 200 VUs
  heavy: {
    plain_heavy: {
      executor: 'constant-arrival-rate',
      rate: 2000, timeUnit: '1s', duration: '60s',
      preAllocatedVUs: 200, maxVUs: 600,
      exec: 'plain',
      tags: { scenario: 'plain' },
    },
    auth_heavy: {
      executor: 'constant-arrival-rate',
      rate: 1000, timeUnit: '1s', duration: '60s',
      preAllocatedVUs: 200, maxVUs: 600,
      startTime: '65s',
      exec: 'auth',
      tags: { scenario: 'auth' },
    },
  },

  // Stress — ramp to breaking point
  stress: {
    stress_ramp: {
      executor: 'ramping-arrival-rate',
      startRate: 500,
      timeUnit: '1s',
      stages: [
        { target: 1000, duration: '30s' },
        { target: 5000, duration: '30s' },
        { target: 10000, duration: '30s' },
        { target: 15000, duration: '30s' },  // saturation zone
        { target: 0, duration: '30s' },  // recovery
      ],
      preAllocatedVUs: 500,
      maxVUs: 1500,
      exec: 'plain',
      tags: { scenario: 'stress' },
    },
  },

  // Spike — sudden 20× traffic burst
  spike: {
    spike_test: {
      executor: 'ramping-arrival-rate',
      startRate: 200,
      timeUnit: '1s',
      stages: [
        { target: 200, duration: '20s' },   // baseline
        { target: 10000, duration: '5s' },   // spike!
        { target: 10000, duration: '20s' },   // sustained burst
        { target: 200, duration: '10s' },   // recovery
        { target: 200, duration: '20s' },   // back to baseline
      ],
      preAllocatedVUs: 500,
      maxVUs: 1500,
      exec: 'plain',
      tags: { scenario: 'spike' },
    },
  },

  // Soak — long-running stability test
  soak: {
    soak_plain: {
      executor: 'constant-arrival-rate',
      rate: 1000, timeUnit: '1s', duration: '10m',
      preAllocatedVUs: 200, maxVUs: 600,
      exec: 'plain',
      tags: { scenario: 'soak' },
    },
  },
};

// ── Options ───────────────────────────────────────────────────
export const options = {
  scenarios: SCENARIOS[SCENARIO] || SCENARIOS.heavy,

  thresholds: {
    'latency_plain_ms': [
      'p(95)<100',   // 95th pct under 100ms
      'p(99)<250',   // 99th pct under 250ms
    ],
    'latency_auth_ms': [
      'p(95)<150',
      'p(99)<300',
    ],
    'errors': ['rate<0.01'],   // <1% errors
    'http_req_failed': ['rate<0.01'],
  },

  // Tag every metric with which gateway is under test
  tags: { target: TARGET, scenario: SCENARIO },

  // Push to Prometheus/Grafana if running in Docker
  ...(IN_DOCKER && {
    ext: {
      loadimpact: { projectID: 0, name: `ando-vs-apisix-${TARGET}` },
    },
  }),
};

// ── Request functions ─────────────────────────────────────────
const PLAIN_PARAMS = { timeout: '2s' };
const AUTH_PARAMS = {
  headers: { apikey: API_KEY },
  timeout: '2s',
};

export function plain() {
  const res = http.get(`${BASE_URL}/bench/plain`, PLAIN_PARAMS);
  const ok = check(res, {
    'status 200': (r) => r.status === 200,
    'body not empty': (r) => r.body && r.body.length > 0,
    'latency < 500ms': (r) => r.timings.duration < 500,
  });
  latencyPlain.add(res.timings.duration, { target: TARGET });
  errorRate.add(!ok, { target: TARGET });
  reqCount.add(1, { target: TARGET, route: 'plain' });
}

export function auth() {
  const res = http.get(`${BASE_URL}/bench/auth`, AUTH_PARAMS);
  const ok = check(res, {
    'status 200': (r) => r.status === 200,
    'not 401': (r) => r.status !== 401,
    'not 403': (r) => r.status !== 403,
  });
  latencyAuth.add(res.timings.duration, { target: TARGET });
  errorRate.add(!ok, { target: TARGET });
  reqCount.add(1, { target: TARGET, route: 'auth' });
}

export default function () {
  plain();
}

// ── Setup / teardown ──────────────────────────────────────────
export function setup() {
  // Validate gateway is reachable before starting
  const res = http.get(`${BASE_URL}/bench/plain`, { timeout: '5s' });
  if (res.status !== 200) {
    throw new Error(`Gateway ${TARGET} at ${BASE_URL} not ready: HTTP ${res.status}`);
  }
  console.log(`✓ ${TARGET.toUpperCase()} at ${BASE_URL} is ready. Scenario: ${SCENARIO}`);
}

// ── Summary ───────────────────────────────────────────────────
export function handleSummary(data) {
  const ts = new Date().toISOString().replace(/[:.]/g, '-');
  const file = `/results/k6_${TARGET}_${SCENARIO}_${ts}.json`;

  return {
    [file]: JSON.stringify(data, null, 2),
    stdout: fmtSummary(data),
  };
}

function fmt(v) { return v !== undefined ? v.toFixed(2) : 'N/A'; }

function fmtSummary(data) {
  const m = (data.metrics || {});
  const rps = m['http_reqs'] ? m['http_reqs'].values.rate.toFixed(0) : 'N/A';
  const err = m['errors'] ? (m['errors'].values.rate * 100).toFixed(2) : '0.00';
  const p = (name, pct) => m[name] ? fmt(m[name].values[`p(${pct})`]) + 'ms' : 'N/A';

  return `
╔══════════════════════════════════════════════╗
║  k6 Result — ${TARGET.toUpperCase().padEnd(6)} | Scenario: ${SCENARIO.padEnd(7)}     ║
╠══════════════════════════════════════════════╣
║  Total RPS:        ${rps.padEnd(26)}║
║  Error rate:       ${(err + '%').padEnd(26)}║
╠══════════════════════════════════════════════╣
║  Plain p50:        ${p('latency_plain_ms', 50).padEnd(26)}║
║  Plain p95:        ${p('latency_plain_ms', 95).padEnd(26)}║
║  Plain p99:        ${p('latency_plain_ms', 99).padEnd(26)}║
╠══════════════════════════════════════════════╣
║  Auth  p50:        ${p('latency_auth_ms', 50).padEnd(26)}║
║  Auth  p95:        ${p('latency_auth_ms', 95).padEnd(26)}║
║  Auth  p99:        ${p('latency_auth_ms', 99).padEnd(26)}║
╚══════════════════════════════════════════════╝
`;
}
