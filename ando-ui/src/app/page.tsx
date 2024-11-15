'use client';

import { useEffect, useState } from 'react';
import { health, HealthResponse, routes as routesApi, upstreams as upstreamsApi, ListResponse, Route, Upstream } from '@/lib/api';

export default function OverviewPage() {
  const [status, setStatus] = useState<HealthResponse | null>(null);
  const [routeData, setRouteData] = useState<ListResponse<Route> | null>(null);
  const [upstreamData, setUpstreamData] = useState<ListResponse<Upstream> | null>(null);
  const [offline, setOffline] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    Promise.all([
      health.check(),
      routesApi.list(),
      upstreamsApi.list(),
    ]).then(([h, r, u]) => {
      setStatus(h);
      setRouteData(r);
      setUpstreamData(u);
      setOffline(false);
    }).catch(() => {
      setOffline(true);
    }).finally(() => setLoading(false));
  }, []);

  if (loading) {
    return (
      <>
        <div className="page-header">
          <div><h1 className="page-title">Overview</h1></div>
        </div>
        <div className="loading-spinner"><div className="spinner" /></div>
      </>
    );
  }

  if (offline || !status) {
    return (
      <>
        <div className="page-header">
          <div>
            <h1 className="page-title">Overview</h1>
            <p className="page-subtitle">System status and cluster telemetry.</p>
          </div>
        </div>
        <div className="card" style={{ border: '1px solid rgba(255,77,106,0.2)', background: 'var(--red-dim)', marginBottom: 24 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 12 }}>
            <span style={{ fontSize: 24 }}>⚠</span>
            <div>
              <h3 style={{ fontSize: 15, fontWeight: 700, color: 'var(--red)', marginBottom: 2 }}>Cannot connect to Ando</h3>
              <p style={{ fontSize: 13, color: 'var(--text-secondary)' }}>The Admin API at port 9180 is not reachable.</p>
            </div>
          </div>
          <div style={{ background: 'var(--bg-app)', borderRadius: 8, padding: 16, fontSize: 12 }}>
            <div style={{ marginBottom: 8, fontWeight: 600, color: 'var(--text-secondary)' }}>Start Ando to connect:</div>
            <pre className="json-display" style={{ marginBottom: 0 }}>{`# Option 1: Docker Compose (full stack)
cd deploy/docker && docker compose up -d

# Option 2: Local development
cargo run -p ando-server -- --config config/ando.yaml`}</pre>
          </div>
        </div>
        <div className="card" style={{ background: 'var(--bg-surface)' }}>
          <h3 style={{ fontSize: 15, fontWeight: 700, marginBottom: 12 }}>Quick Links</h3>
          <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
            <a href="/routes" className="btn">⇢ Routes</a>
            <a href="/upstreams" className="btn">⬡ Upstreams</a>
            <a href="/plugins" className="btn">⧉ Plugins</a>
            <a href="/observability" className="btn">◑ Observability</a>
          </div>
        </div>
      </>
    );
  }

  return (
    <>
      <div className="page-header">
        <div>
          <h1 className="page-title">Overview</h1>
          <p className="page-subtitle">Real-time telemetry from your Ando API Gateway cluster.</p>
        </div>
        <a href="/observability" className="btn">◑ Metrics</a>
      </div>

      <div className="stats-grid">
        <div className="card stat-card">
          <div className="stat-label">Gateway Status</div>
          <div className="stat-value" style={{ color: 'var(--green)', fontSize: 22, fontWeight: 800 }}>● Online</div>
          <div className="stat-sub">Version {status.version}</div>
        </div>
        <div className="card stat-card">
          <div className="stat-label">Active Routes</div>
          <div className="stat-value">{status.cache.routes}</div>
          <div className="stat-sub">Routing rules in cache</div>
        </div>
        <div className="card stat-card">
          <div className="stat-label">Upstreams</div>
          <div className="stat-value">{status.cache.upstreams}</div>
          <div className="stat-sub">Backend server pools</div>
        </div>
        <div className="card stat-card">
          <div className="stat-label">Plugins Loaded</div>
          <div className="stat-value" style={{ color: 'var(--indigo)' }}>{status.plugins_loaded}</div>
          <div className="stat-sub">Rust + Lua plugins active</div>
        </div>
      </div>

      <div className="stats-grid">
        <div className="card stat-card">
          <div className="stat-label">Services</div>
          <div className="stat-value">{status.cache.services}</div>
          <div className="stat-sub">Shared plugin configs</div>
        </div>
        <div className="card stat-card">
          <div className="stat-label">Consumers</div>
          <div className="stat-value">{status.cache.consumers}</div>
          <div className="stat-sub">Authenticated API clients</div>
        </div>
        <div className="card stat-card">
          <div className="stat-label">SSL Certificates</div>
          <div className="stat-value">{status.cache.ssl_certs}</div>
          <div className="stat-sub">TLS/SNI certificates</div>
        </div>
        <div className="card stat-card">
          <div className="stat-label">Plugin Configs</div>
          <div className="stat-value">{status.cache.plugin_configs}</div>
          <div className="stat-sub">Reusable plugin sets</div>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 20, marginBottom: 24 }}>
        {/* Recent Routes */}
        <div className="card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
            <h3 style={{ fontSize: 15, fontWeight: 700 }}>Recent Routes</h3>
            <a href="/routes" className="btn btn-sm">View All</a>
          </div>
          {routeData && routeData.list.length > 0 ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {routeData.list.slice(0, 5).map((r) => (
                <div key={r.id} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid var(--border)' }}>
                  <div>
                    <div style={{ fontWeight: 600, fontSize: 13 }}>{r.name || r.id}</div>
                    <code className="code-inline" style={{ fontSize: 11 }}>{r.uri}</code>
                  </div>
                  <span className={`badge ${r.status === 1 && r.enable ? 'badge-green' : 'badge-red'}`}>
                    {r.status === 1 && r.enable ? 'Active' : 'Disabled'}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <p style={{ color: 'var(--text-tertiary)', fontSize: 13 }}>No routes configured. <a href="/routes" style={{ color: 'var(--cyan)' }}>Create one →</a></p>
          )}
        </div>

        {/* Recent Upstreams */}
        <div className="card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
            <h3 style={{ fontSize: 15, fontWeight: 700 }}>Upstreams</h3>
            <a href="/upstreams" className="btn btn-sm">View All</a>
          </div>
          {upstreamData && upstreamData.list.length > 0 ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {upstreamData.list.slice(0, 5).map((u) => (
                <div key={u.id} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid var(--border)' }}>
                  <div>
                    <div style={{ fontWeight: 600, fontSize: 13 }}>{u.name || u.id}</div>
                    <span className="badge badge-muted" style={{ fontSize: 10 }}>{u.type} · {Object.keys(u.nodes).length} node{Object.keys(u.nodes).length !== 1 ? 's' : ''}</span>
                  </div>
                  <span className="badge badge-indigo">{u.scheme}</span>
                </div>
              ))}
            </div>
          ) : (
            <p style={{ color: 'var(--text-tertiary)', fontSize: 13 }}>No upstreams configured. <a href="/upstreams" style={{ color: 'var(--cyan)' }}>Create one →</a></p>
          )}
        </div>
      </div>

      {/* Quick links */}
      <div className="card">
        <h3 style={{ fontSize: 15, fontWeight: 700, marginBottom: 12 }}>Quick Actions</h3>
        <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
          <a href="/routes" className="btn">⇢ Manage Routes</a>
          <a href="/upstreams" className="btn">⬡ Manage Upstreams</a>
          <a href="/consumers" className="btn">⊕ Manage Consumers</a>
          <a href="/ssl" className="btn">⊘ SSL Certificates</a>
          <a href="/plugins" className="btn">⧉ Browse Plugins</a>
          <a href="/observability" className="btn">◑ Observability</a>
        </div>
      </div>
    </>
  );
}
