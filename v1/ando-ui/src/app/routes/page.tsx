'use client';

import { useEffect, useState, useCallback } from 'react';
import { routes as api, Route, upstreams as upstreamApi, ListResponse, Upstream } from '@/lib/api';
import Modal from '@/components/Modal';

const METHODS = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS'];

export default function RoutesPage() {
    const [data, setData] = useState<ListResponse<Route> | null>(null);
    const [upstreamList, setUpstreamList] = useState<Upstream[]>([]);
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(true);
    const [showCreate, setShowCreate] = useState(false);
    const [editing, setEditing] = useState<Route | null>(null);
    const [confirmDelete, setConfirmDelete] = useState<Route | null>(null);

    const load = useCallback(() => {
        setLoading(true);
        Promise.all([api.list(), upstreamApi.list()])
            .then(([r, u]) => { setData(r); setUpstreamList(u.list); })
            .catch((e) => setError(e.message))
            .finally(() => setLoading(false));
    }, []);

    useEffect(() => { load(); }, [load]);

    const handleDelete = async (id: string) => {
        await api.delete(id);
        setConfirmDelete(null);
        load();
    };

    return (
        <>
            <div className="page-header">
                <div>
                    <h1 className="page-title">Routes</h1>
                    <p className="page-subtitle">Define URI matching rules and plugin pipelines for incoming requests.</p>
                </div>
                <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Route</button>
            </div>

            {error && <div className="error-banner">⚠ {error}</div>}

            {loading ? (
                <div className="loading-spinner"><div className="spinner" /></div>
            ) : data && data.list.length > 0 ? (
                <div className="table-wrapper">
                    <table className="table">
                        <thead>
                            <tr>
                                <th>Name / URI</th>
                                <th>Methods</th>
                                <th>Upstream</th>
                                <th>Plugins</th>
                                <th>Status</th>
                                <th style={{ textAlign: 'right' }}>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {data.list.map((route) => (
                                <tr key={route.id}>
                                    <td>
                                        <div style={{ fontWeight: 600, marginBottom: 3 }}>{route.name || route.id}</div>
                                        <code className="code-inline">{route.uri}</code>
                                        {route.host && <span className="badge badge-muted" style={{ marginLeft: 6 }}>{route.host}</span>}
                                    </td>
                                    <td>
                                        <div className="tag-list">
                                            {route.methods.length > 0
                                                ? route.methods.map((m) => <span key={m} className={`method-badge method-${m}`}>{m}</span>)
                                                : <span className="method-badge method-ANY">ANY</span>
                                            }
                                        </div>
                                    </td>
                                    <td>
                                        {route.upstream_id
                                            ? <span className="badge badge-cyan">{route.upstream_id}</span>
                                            : route.upstream
                                                ? <span className="badge badge-indigo">inline</span>
                                                : route.service_id
                                                    ? <span className="badge badge-yellow">svc:{route.service_id}</span>
                                                    : <span className="badge badge-muted">none</span>
                                        }
                                    </td>
                                    <td>
                                        {Object.keys(route.plugins).length > 0
                                            ? <span className="badge badge-indigo">{Object.keys(route.plugins).length} plugin{Object.keys(route.plugins).length > 1 ? 's' : ''}</span>
                                            : <span style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>—</span>
                                        }
                                    </td>
                                    <td>
                                        <span className={`badge ${route.status === 1 && route.enable ? 'badge-green' : 'badge-red'}`}>
                                            {route.status === 1 && route.enable ? 'Active' : 'Disabled'}
                                        </span>
                                    </td>
                                    <td>
                                        <div className="actions-cell">
                                            <button className="btn btn-sm btn-icon" onClick={() => setEditing(route)} title="Edit">✎</button>
                                            <button className="btn btn-sm btn-icon btn-danger" onClick={() => setConfirmDelete(route)} title="Delete">✕</button>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            ) : (
                <div className="card empty-state">
                    <div className="empty-state-icon">⇢</div>
                    <div className="empty-state-title">No routes configured</div>
                    <div className="empty-state-desc">Create your first route to start proxying traffic.</div>
                    <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Route</button>
                </div>
            )}

            <RouteFormModal
                open={showCreate}
                onClose={() => setShowCreate(false)}
                onSaved={load}
                upstreams={upstreamList}
            />

            {editing && (
                <RouteFormModal
                    open={true}
                    onClose={() => setEditing(null)}
                    onSaved={() => { setEditing(null); load(); }}
                    initial={editing}
                    upstreams={upstreamList}
                />
            )}

            {confirmDelete && (
                <Modal title="Delete Route" open={true} onClose={() => setConfirmDelete(null)} footer={
                    <>
                        <button className="btn" onClick={() => setConfirmDelete(null)}>Cancel</button>
                        <button className="btn btn-danger" onClick={() => handleDelete(confirmDelete.id)}>Delete</button>
                    </>
                }>
                    <p style={{ fontSize: 13, lineHeight: 1.7 }}>
                        Are you sure you want to delete route <strong>{confirmDelete.name || confirmDelete.id}</strong>
                        {' '}(<code className="code-inline">{confirmDelete.uri}</code>)? This action cannot be undone.
                    </p>
                </Modal>
            )}
        </>
    );
}

function RouteFormModal({ open, onClose, onSaved, initial, upstreams }: {
    open: boolean; onClose: () => void; onSaved: () => void;
    initial?: Route; upstreams: Upstream[];
}) {
    const isEdit = !!initial;
    const [form, setForm] = useState({
        name: '', uri: '', methods: [] as string[],
        host: '', upstream_id: '', service_id: '', priority: 0,
        enable: true, status: 1,
        plugins: '{}',
        nodes: '' as string,
    });
    const [saving, setSaving] = useState(false);
    const [err, setErr] = useState('');

    useEffect(() => {
        if (initial) {
            setForm({
                name: initial.name || '',
                uri: initial.uri,
                methods: initial.methods || [],
                host: initial.host || '',
                upstream_id: initial.upstream_id || '',
                service_id: initial.service_id || '',
                priority: initial.priority,
                enable: initial.enable,
                status: initial.status,
                plugins: JSON.stringify(initial.plugins || {}, null, 2),
                nodes: initial.upstream ? Object.entries(initial.upstream.nodes).map(([k, v]) => `${k}:${v}`).join('\n') : '',
            });
        } else {
            setForm({ name: '', uri: '', methods: [], host: '', upstream_id: '', service_id: '', priority: 0, enable: true, status: 1, plugins: '{}', nodes: '' });
        }
    }, [initial, open]);

    const toggleMethod = (m: string) => {
        setForm((f) => ({
            ...f,
            methods: f.methods.includes(m) ? f.methods.filter((x) => x !== m) : [...f.methods, m],
        }));
    };

    const handleSave = async () => {
        setSaving(true);
        setErr('');
        try {
            let parsedPlugins = {};
            try { parsedPlugins = JSON.parse(form.plugins); } catch { throw new Error('Invalid plugin JSON'); }

            const payload: any = {
                name: form.name,
                uri: form.uri,
                methods: form.methods.length > 0 ? form.methods : undefined,
                host: form.host || undefined,
                upstream_id: form.upstream_id || undefined,
                service_id: form.service_id || undefined,
                priority: form.priority,
                enable: form.enable,
                status: form.status,
                plugins: parsedPlugins,
            };

            if (form.nodes && !form.upstream_id) {
                const nodes: Record<string, number> = {};
                form.nodes.split('\n').filter(Boolean).forEach((line) => {
                    const parts = line.split(':');
                    const weight = parseInt(parts[parts.length - 1]) || 1;
                    const addr = parts.slice(0, -1).join(':') || parts[0];
                    nodes[addr] = weight;
                });
                if (Object.keys(nodes).length > 0) {
                    payload.upstream = { type: 'roundrobin', nodes };
                }
            }

            if (isEdit) {
                await api.update(initial!.id, payload);
            } else {
                await api.create(payload);
            }
            onSaved();
            onClose();
        } catch (e: any) {
            setErr(e.message);
        } finally {
            setSaving(false);
        }
    };

    return (
        <Modal title={isEdit ? 'Edit Route' : 'Create Route'} open={open} onClose={onClose} large footer={
            <>
                <button className="btn" onClick={onClose}>Cancel</button>
                <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
                    {saving ? 'Saving…' : isEdit ? 'Update Route' : 'Create Route'}
                </button>
            </>
        }>
            {err && <div className="error-banner" style={{ marginBottom: 16 }}>⚠ {err}</div>}

            <div className="form-row-inline">
                <div className="form-row">
                    <label className="form-label">Name</label>
                    <input className="form-input" placeholder="my-api-route" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
                </div>
                <div className="form-row">
                    <label className="form-label">Priority</label>
                    <input className="form-input" type="number" value={form.priority} onChange={(e) => setForm({ ...form, priority: parseInt(e.target.value) || 0 })} />
                </div>
            </div>

            <div className="form-row">
                <label className="form-label">URI Pattern *</label>
                <input className="form-input" placeholder="/api/v1/users/*" value={form.uri} onChange={(e) => setForm({ ...form, uri: e.target.value })} />
                <div className="form-hint">Supports exact, prefix (/*) and parametric (/:param) matching.</div>
            </div>

            <div className="form-row">
                <label className="form-label">HTTP Methods</label>
                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                    {METHODS.map((m) => (
                        <button
                            key={m}
                            type="button"
                            className={`method-badge method-${m}`}
                            style={{
                                cursor: 'pointer',
                                opacity: form.methods.includes(m) ? 1 : 0.3,
                                transform: form.methods.includes(m) ? 'scale(1.05)' : 'scale(1)',
                                transition: 'all 0.15s',
                                padding: '4px 10px',
                            }}
                            onClick={() => toggleMethod(m)}
                        >
                            {m}
                        </button>
                    ))}
                </div>
                <div className="form-hint">Leave empty for all methods.</div>
            </div>

            <div className="form-row">
                <label className="form-label">Host</label>
                <input className="form-input" placeholder="api.example.com" value={form.host} onChange={(e) => setForm({ ...form, host: e.target.value })} />
            </div>

            <div className="form-row-inline">
                <div className="form-row">
                    <label className="form-label">Upstream</label>
                    <select className="form-input form-select" value={form.upstream_id} onChange={(e) => setForm({ ...form, upstream_id: e.target.value })}>
                        <option value="">— Select upstream —</option>
                        {upstreams.map((u) => <option key={u.id} value={u.id}>{u.name || u.id}</option>)}
                    </select>
                </div>
                <div className="form-row">
                    <label className="form-label">Service ID</label>
                    <input className="form-input" placeholder="Optional service reference" value={form.service_id} onChange={(e) => setForm({ ...form, service_id: e.target.value })} />
                </div>
            </div>

            {!form.upstream_id && (
                <div className="form-row">
                    <label className="form-label">Inline Upstream Nodes</label>
                    <textarea
                        className="form-input form-textarea"
                        placeholder={"127.0.0.1:8080:1\n192.168.1.10:8080:2"}
                        value={form.nodes}
                        onChange={(e) => setForm({ ...form, nodes: e.target.value })}
                        style={{ minHeight: 80 }}
                    />
                    <div className="form-hint">Format: host:port:weight (one per line). Only used if no upstream is selected.</div>
                </div>
            )}

            <div className="form-row">
                <label className="form-label">Plugins (JSON)</label>
                <textarea
                    className="form-input form-textarea"
                    value={form.plugins}
                    onChange={(e) => setForm({ ...form, plugins: e.target.value })}
                />
                <div className="form-hint">Example: {`{"limit-count": {"count": 100, "time_window": 60}}`}</div>
            </div>

            <div className="form-row" style={{ display: 'flex', gap: 24 }}>
                <label className="form-checkbox">
                    <input type="checkbox" checked={form.enable} onChange={(e) => setForm({ ...form, enable: e.target.checked })} />
                    Enabled
                </label>
                <label className="form-checkbox">
                    <input type="checkbox" checked={form.status === 1} onChange={(e) => setForm({ ...form, status: e.target.checked ? 1 : 0 })} />
                    Active Status
                </label>
            </div>
        </Modal>
    );
}
