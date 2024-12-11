'use client';

import { useEffect, useState } from 'react';
import { plugins as api } from '@/lib/api';

const PLUGIN_META: Record<string, { category: string; description: string; phases: string[] }> = {
    'key-auth': { category: 'Authentication', description: 'API key authentication via header or query parameter.', phases: ['access'] },
    'jwt-auth': { category: 'Authentication', description: 'JWT token validation with HS256/RS256 support.', phases: ['access'] },
    'basic-auth': { category: 'Authentication', description: 'HTTP Basic authentication with bcrypt passwords.', phases: ['access'] },
    'limit-count': { category: 'Traffic Control', description: 'Fixed-window rate limiting per consumer or IP.', phases: ['access'] },
    'limit-req': { category: 'Traffic Control', description: 'Leaky bucket rate limiter for precise throttling.', phases: ['access'] },
    'cors': { category: 'Transform', description: 'Cross-Origin Resource Sharing preflight and headers.', phases: ['rewrite', 'header_filter'] },
    'request-transformer': { category: 'Transform', description: 'Add, remove, or rename request headers and body.', phases: ['rewrite'] },
    'response-transformer': { category: 'Transform', description: 'Modify response headers before sending to client.', phases: ['header_filter'] },
    'ip-restriction': { category: 'Security', description: 'CIDR-based IP allow/deny lists for access control.', phases: ['access'] },
};

const CATEGORY_COLORS: Record<string, string> = {
    'Authentication': 'badge-cyan',
    'Traffic Control': 'badge-yellow',
    'Transform': 'badge-indigo',
    'Security': 'badge-red',
};

export default function PluginsPage() {
    const [pluginNames, setPluginNames] = useState<string[]>([]);
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(true);
    const [filter, setFilter] = useState('All');

    useEffect(() => {
        api.list()
            .then((d) => setPluginNames(d.list))
            .catch((e) => setError(e.message))
            .finally(() => setLoading(false));
    }, []);

    const categories = ['All', ...Array.from(new Set(Object.values(PLUGIN_META).map((m) => m.category)))];

    const filtered = pluginNames.filter((name) => {
        if (filter === 'All') return true;
        return PLUGIN_META[name]?.category === filter;
    });

    return (
        <>
            <div className="page-header">
                <div>
                    <h1 className="page-title">Plugins</h1>
                    <p className="page-subtitle">Browse all registered plugins (Rust built-in + Lua) available for routes and services.</p>
                </div>
            </div>

            {error && <div className="error-banner">⚠ {error}</div>}

            <div style={{ marginBottom: 24 }}>
                <div className="pill-tabs">
                    {categories.map((cat) => (
                        <button key={cat} className={`pill-tab ${filter === cat ? 'active' : ''}`} onClick={() => setFilter(cat)}>{cat}</button>
                    ))}
                </div>
            </div>

            {loading ? (
                <div className="loading-spinner"><div className="spinner" /></div>
            ) : (
                <div className="plugin-grid">
                    {filtered.map((name) => {
                        const meta = PLUGIN_META[name];
                        return (
                            <div key={name} className="card card-hover plugin-card">
                                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 8 }}>
                                    <div className="plugin-card-name">{name}</div>
                                    <span className={`badge ${CATEGORY_COLORS[meta?.category || ''] || 'badge-muted'}`}>
                                        {meta?.category || 'Custom'}
                                    </span>
                                </div>
                                <p className="plugin-card-category">{meta?.description || 'Custom / Lua plugin'}</p>
                                <div className="plugin-card-phases">
                                    {(meta?.phases || ['access']).map((p) => (
                                        <span key={p} className="badge badge-muted" style={{ fontSize: 10 }}>{p}</span>
                                    ))}
                                </div>

                                <div style={{ marginTop: 16, paddingTop: 12, borderTop: '1px solid var(--border)' }}>
                                    <div className="form-label" style={{ marginBottom: 4 }}>Usage example</div>
                                    <code className="code-inline" style={{ fontSize: 11, display: 'block' }}>
                                        {`"plugins": { "${name}": { ... } }`}
                                    </code>
                                </div>
                            </div>
                        );
                    })}
                </div>
            )}

            {!loading && filtered.length === 0 && (
                <div className="card empty-state">
                    <div className="empty-state-icon">⧉</div>
                    <div className="empty-state-title">No plugins found</div>
                    <div className="empty-state-desc">No plugins match the current filter.</div>
                </div>
            )}
        </>
    );
}
