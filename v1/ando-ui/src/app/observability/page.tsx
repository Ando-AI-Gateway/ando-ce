'use client';

import { useEffect, useState } from 'react';
import { metrics, health, HealthResponse } from '@/lib/api';

interface ParsedMetric {
    name: string;
    help: string;
    type: string;
    values: { labels: string; value: string }[];
}

function parsePrometheus(text: string): ParsedMetric[] {
    const result: ParsedMetric[] = [];
    const lines = text.split('\n');
    let current: ParsedMetric | null = null;

    for (const line of lines) {
        if (line.startsWith('# HELP')) {
            const parts = line.slice(7).split(' ');
            const name = parts[0];
            const help = parts.slice(1).join(' ');
            current = { name, help, type: '', values: [] };
            result.push(current);
        } else if (line.startsWith('# TYPE')) {
            if (current) current.type = line.split(' ')[3] || '';
        } else if (line && !line.startsWith('#') && current) {
            const braceIdx = line.indexOf('{');
            const spaceIdx = line.lastIndexOf(' ');
            const value = line.slice(spaceIdx + 1);
            const labels = braceIdx !== -1 ? line.slice(braceIdx, spaceIdx) : '';
            current.values.push({ labels, value });
        }
    }

    return result.filter((m) => m.values.length > 0);
}

const INTERESTING_METRICS = [
    'ando_requests_total',
    'ando_request_duration_seconds',
    'ando_upstream_latency_seconds',
    'ando_active_connections',
    'ando_plugin_executions_total',
];

export default function ObservabilityPage() {
    const [raw, setRaw] = useState('');
    const [parsed, setParsed] = useState<ParsedMetric[]>([]);
    const [health, setHealth] = useState<HealthResponse | null>(null);
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(true);

    const load = async () => {
        setLoading(true);
        try {
            const [m, h] = await Promise.all([metrics.get(), import('@/lib/api').then((a) => a.health.check())]);
            setRaw(m);
            setParsed(parsePrometheus(m));
            setHealth(h);
        } catch (e: any) {
            setError(e.message);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => { load(); }, []);

    const interesting = parsed.filter((m) => INTERESTING_METRICS.some((i) => m.name.startsWith(i)));
    const all = parsed;

    return (
        <>
            <div className="page-header">
                <div>
                    <h1 className="page-title">Observability</h1>
                    <p className="page-subtitle">Live Prometheus metrics from your Ando node. Scrape endpoint at <code className="code-inline">/metrics</code>.</p>
                </div>
                <button className="btn" onClick={load}>↻ Refresh</button>
            </div>

            {error && <div className="error-banner">⚠ {error} — Is Ando running?</div>}

            {/* Health Summary */}
            {health && (
                <div className="stats-grid" style={{ marginBottom: 32 }}>
                    <div className="card stat-card">
                        <div className="stat-label">Status</div>
                        <div className="stat-value" style={{ color: 'var(--green)' }}>OK</div>
                        <div className="stat-sub">Version {health.version}</div>
                    </div>
                    <div className="card stat-card">
                        <div className="stat-label">Routes Cached</div>
                        <div className="stat-value">{health.cache.routes}</div>
                        <div className="stat-sub">In-memory config</div>
                    </div>
                    <div className="card stat-card">
                        <div className="stat-label">Plugins Loaded</div>
                        <div className="stat-value" style={{ color: 'var(--indigo)' }}>{health.plugins_loaded}</div>
                        <div className="stat-sub">Rust + Lua runtime</div>
                    </div>
                    <div className="card stat-card">
                        <div className="stat-label">SSL Certs</div>
                        <div className="stat-value">{health.cache.ssl_certs}</div>
                        <div className="stat-sub">Active TLS certs</div>
                    </div>
                </div>
            )}

            {loading ? (
                <div className="loading-spinner"><div className="spinner" /></div>
            ) : (
                <>
                    {/* Key Metrics */}
                    {interesting.length > 0 && (
                        <div style={{ marginBottom: 32 }}>
                            <h2 style={{ fontSize: 16, fontWeight: 700, marginBottom: 16 }}>Key Metrics</h2>
                            <div className="plugin-grid">
                                {interesting.map((m) => (
                                    <div key={m.name} className="card card-hover">
                                        <div style={{ fontWeight: 700, fontSize: 13, marginBottom: 4, color: 'var(--cyan)' }}>
                                            {m.name}
                                        </div>
                                        <div style={{ fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 12 }}>{m.help}</div>
                                        <div style={{ fontSize: 11 }}>
                                            {m.values.slice(0, 5).map((v, i) => (
                                                <div key={i} style={{ display: 'flex', justifyContent: 'space-between', padding: '4px 0', borderBottom: '1px solid var(--border)' }}>
                                                    <code style={{ color: 'var(--text-secondary)', fontSize: 10 }}>{v.labels || 'total'}</code>
                                                    <span style={{ fontWeight: 600 }}>{v.value}</span>
                                                </div>
                                            ))}
                                        </div>
                                        <span className="badge badge-muted" style={{ marginTop: 10 }}>{m.type}</span>
                                    </div>
                                ))}
                            </div>
                        </div>
                    )}

                    {/* Raw Prometheus Output */}
                    <div className="card">
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
                            <h2 style={{ fontSize: 16, fontWeight: 700 }}>Raw Prometheus Output</h2>
                            <span className="badge badge-muted">{all.length} metrics</span>
                        </div>
                        {raw ? (
                            <pre className="json-display" style={{ maxHeight: 500, fontSize: 11 }}>{raw}</pre>
                        ) : (
                            <div className="empty-state" style={{ padding: 32 }}>
                                <div className="empty-state-icon">◎</div>
                                <div className="empty-state-title">No metrics available</div>
                                <div className="empty-state-desc">Start Ando to see Prometheus metrics here.</div>
                            </div>
                        )}
                    </div>
                </>
            )}
        </>
    );
}
