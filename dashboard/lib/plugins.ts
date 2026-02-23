export interface PluginInfo {
  name: string;
  phase: string;
  icon: string;
  desc: string;
  features?: string[];
}

export const CE_PLUGINS: PluginInfo[] = [
  { name: "key-auth", phase: "access", icon: "key", desc: "API key authentication via header or query string" },
  { name: "jwt-auth", phase: "access", icon: "shield", desc: "JWT token validation with configurable claims" },
  { name: "basic-auth", phase: "access", icon: "user", desc: "HTTP Basic authentication against consumer credentials" },
  { name: "ip-restriction", phase: "access", icon: "globe", desc: "Allow/deny lists based on client IP or CIDR range" },
  { name: "rate-limiting", phase: "access", icon: "activity", desc: "Request rate limits per route or consumer (in-memory counter)" },
  { name: "cors", phase: "header_filter", icon: "layers", desc: "Cross-Origin Resource Sharing headers for browser clients" },
];

export const EE_PLUGINS: PluginInfo[] = [
  { name: "hmac-auth", phase: "access", icon: "key", desc: "HMAC-signed request authentication with clock-skew protection", features: ["SHA-256 / SHA-512 signatures", "Configurable clock skew tolerance", "Request body signing"] },
  { name: "oauth2", phase: "access", icon: "shield", desc: "Full OAuth 2.0 authorization code and client credentials flow", features: ["Authorization code flow", "Client credentials grant", "Token introspection endpoint", "PKCE support"] },
  { name: "rate-limiting-advanced", phase: "access", icon: "activity", desc: "Distributed rate limiting with sliding window and Redis backend", features: ["Redis-backed counters", "Sliding window algorithm", "Per-consumer quotas", "Burst allowance"] },
  { name: "traffic-mirror", phase: "access", icon: "layers", desc: "Mirror production traffic to staging for shadow testing", features: ["Percentage-based mirroring", "Header-based routing", "Async fire-and-forget", "Response comparison"] },
  { name: "canary-release", phase: "access", icon: "layers", desc: "Gradual traffic shifting between upstream versions", features: ["Weight-based split", "Header-based canary", "Cookie-based sticky sessions", "Auto-rollback on error rate"] },
  { name: "circuit-breaker", phase: "access", icon: "activity", desc: "Automatic upstream failure detection and recovery", features: ["Configurable failure threshold", "Half-open probing", "Per-upstream state machine", "Prometheus metrics export"] },
];

export const COMPARISON_ROWS = [
  { feature: "Open-source core (monoio, io_uring)", ce: true, ee: true },
  { feature: "Routes / Upstreams / Consumers CRUD", ce: true, ee: true },
  { feature: "key-auth, jwt-auth, basic-auth", ce: true, ee: true },
  { feature: "ip-restriction", ce: true, ee: true },
  { feature: "rate-limiting (in-memory)", ce: true, ee: true },
  { feature: "CORS plugin", ce: true, ee: true },
  { feature: "Security headers (HSTS, CSP, …)", ce: true, ee: true },
  { feature: "Prometheus metrics", ce: true, ee: true },
  { feature: "Admin REST API", ce: true, ee: true },
  { feature: "Built-in Dashboard", ce: true, ee: true },
  { feature: "hmac-auth / oauth2", ce: false, ee: true },
  { feature: "rate-limiting-advanced (Redis)", ce: false, ee: true },
  { feature: "traffic-mirror / canary-release", ce: false, ee: true },
  { feature: "circuit-breaker", ce: false, ee: true },
  { feature: "Multi-node clustering", ce: false, ee: true },
  { feature: "RBAC for Admin API", ce: false, ee: true },
];
