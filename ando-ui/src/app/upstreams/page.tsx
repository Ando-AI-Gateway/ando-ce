'use client';

import { useEffect, useState, useCallback } from 'react';
import { upstreams as api, Upstream, ListResponse } from '@/lib/api';
import Modal from '@/components/Modal';

const LB_TYPES = ['roundrobin', 'chash', 'ewma', 'least_conn'];

export default function UpstreamsPage() {
    const [data, setData] = useState<ListResponse<Upstream> | null>(null);
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(true);
    const [showCreate, setShowCreate] = useState(false);
    const [editing, setEditing] = useState<Upstream | null>(null);
    const [confirmDelete, setConfirmDelete] = useState<Upstream | null>(null);

    const load = useCallback(() => {
        setLoading(true);
        api.list().then(setData).catch((e) => setError(e.message)).finally(() => setLoading(false));
    }, []);

    useEffect(() => { load(); }, [load]);

    return (
        <>
            <div className="page-header">
                <div>
                    <h1 className="page-title">Upstreams</h1>
                    <p className="page-subtitle">Backend server pools with load balancing, health checks, and retry policies.</p>
                </div>
                <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Upstream</button>
            </div>

            {error && <div className="error-banner">⚠ {error}</div>}

            {loading ? (
                <div className="loading-spinner"><div className="spinner" /></div>
            ) : data && data.list.length > 0 ? (
                <div className="table-wrapper">
                    <table className="table">
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Algorithm</th>
                                <th>Nodes</th>
                                <th>Scheme</th>
                                <th>Retries</th>
                                <th style={{ textAlign: 'right' }}>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {data.list.map((u) => (
                                <tr key={u.id}>
                                    <td>
                                        <div style={{ fontWeight: 600, marginBottom: 2 }}>{u.name || u.id}</div>
                                        {u.description && <div style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>{u.description}</div>}
                                    </td>
                                    <td><span className="badge badge-indigo">{u.type}</span></td>
                                    <td>
                                        <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                                            {Object.entries(u.nodes).map(([addr, w]) => (
                                                <div key={addr} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                                                    <code className="code-inline">{addr}</code>
                                                    <span className="badge badge-muted">w:{w}</span>
                                                </div>
                                            ))}
                                        </div>
                                    </td>
                                    <td><span className="badge badge-muted">{u.scheme}</span></td>
                                    <td>{u.retries}</td>
                                    <td>
                                        <div className="actions-cell">
                                            <button className="btn btn-sm btn-icon" onClick={() => setEditing(u)} title="Edit">✎</button>
                                            <button className="btn btn-sm btn-icon btn-danger" onClick={() => setConfirmDelete(u)} title="Delete">✕</button>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            ) : (
                <div className="card empty-state">
                    <div className="empty-state-icon">⬡</div>
                    <div className="empty-state-title">No upstreams configured</div>
                    <div className="empty-state-desc">Upstreams define backend server pools for your routes.</div>
                    <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Upstream</button>
                </div>
            )}

            <UpstreamFormModal open={showCreate} onClose={() => setShowCreate(false)} onSaved={load} />
            {editing && <UpstreamFormModal open={true} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); load(); }} initial={editing} />}
            {confirmDelete && (
                <Modal title="Delete Upstream" open={true} onClose={() => setConfirmDelete(null)} footer={
                    <>
                        <button className="btn" onClick={() => setConfirmDelete(null)}>Cancel</button>
                        <button className="btn btn-danger" onClick={async () => { await api.delete(confirmDelete.id); setConfirmDelete(null); load(); }}>Delete</button>
                    </>
                }>
                    <p style={{ fontSize: 13 }}>Delete upstream <strong>{confirmDelete.name || confirmDelete.id}</strong>? Routes using this upstream will break.</p>
                </Modal>
            )}
        </>
    );
}

function UpstreamFormModal({ open, onClose, onSaved, initial }: { open: boolean; onClose: () => void; onSaved: () => void; initial?: Upstream }) {
    const isEdit = !!initial;
    const [form, setForm] = useState({
        name: '', description: '', type: 'roundrobin', scheme: 'http',
        retries: 1, pass_host: 'pass', upstream_host: '',
        nodes: [{ addr: '', weight: 1 }] as { addr: string; weight: number }[],
    });
    const [saving, setSaving] = useState(false);
    const [err, setErr] = useState('');

    useEffect(() => {
        if (initial) {
            setForm({
                name: initial.name, description: initial.description, type: initial.type,
                scheme: initial.scheme, retries: initial.retries, pass_host: initial.pass_host,
                upstream_host: initial.upstream_host || '',
                nodes: Object.entries(initial.nodes).map(([addr, weight]) => ({ addr, weight })),
            });
        } else {
            setForm({ name: '', description: '', type: 'roundrobin', scheme: 'http', retries: 1, pass_host: 'pass', upstream_host: '', nodes: [{ addr: '', weight: 1 }] });
        }
    }, [initial, open]);

    const addNode = () => setForm({ ...form, nodes: [...form.nodes, { addr: '', weight: 1 }] });
    const removeNode = (i: number) => setForm({ ...form, nodes: form.nodes.filter((_, idx) => idx !== i) });
    const updateNode = (i: number, field: string, val: any) => {
        const nodes = [...form.nodes];
        (nodes[i] as any)[field] = val;
        setForm({ ...form, nodes });
    };

    const handleSave = async () => {
        setSaving(true); setErr('');
        try {
            const nodes: Record<string, number> = {};
            form.nodes.filter((n) => n.addr).forEach((n) => { nodes[n.addr] = n.weight; });
            if (Object.keys(nodes).length === 0) throw new Error('At least one node is required');

            const payload: any = {
                name: form.name, description: form.description, type: form.type,
                scheme: form.scheme, retries: form.retries, pass_host: form.pass_host,
                upstream_host: form.upstream_host || undefined, nodes,
            };
            if (isEdit) await api.update(initial!.id, payload);
            else await api.create(payload);
            onSaved(); onClose();
        } catch (e: any) { setErr(e.message); }
        finally { setSaving(false); }
    };

    return (
        <Modal title={isEdit ? 'Edit Upstream' : 'Create Upstream'} open={open} onClose={onClose} large footer={
            <>
                <button className="btn" onClick={onClose}>Cancel</button>
                <button className="btn btn-primary" onClick={handleSave} disabled={saving}>{saving ? 'Saving…' : isEdit ? 'Update' : 'Create'}</button>
            </>
        }>
            {err && <div className="error-banner">{err}</div>}
            <div className="form-row-inline">
                <div className="form-row"><label className="form-label">Name</label><input className="form-input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} /></div>
                <div className="form-row"><label className="form-label">Description</label><input className="form-input" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} /></div>
            </div>
            <div className="form-row-inline">
                <div className="form-row">
                    <label className="form-label">Load Balancer</label>
                    <select className="form-input form-select" value={form.type} onChange={(e) => setForm({ ...form, type: e.target.value })}>
                        {LB_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
                    </select>
                </div>
                <div className="form-row">
                    <label className="form-label">Scheme</label>
                    <select className="form-input form-select" value={form.scheme} onChange={(e) => setForm({ ...form, scheme: e.target.value })}>
                        {['http', 'https', 'grpc', 'grpcs'].map((s) => <option key={s} value={s}>{s}</option>)}
                    </select>
                </div>
            </div>
            <div className="form-row-inline">
                <div className="form-row"><label className="form-label">Retries</label><input className="form-input" type="number" min={0} value={form.retries} onChange={(e) => setForm({ ...form, retries: parseInt(e.target.value) || 0 })} /></div>
                <div className="form-row">
                    <label className="form-label">Pass Host</label>
                    <select className="form-input form-select" value={form.pass_host} onChange={(e) => setForm({ ...form, pass_host: e.target.value })}>
                        {['pass', 'node', 'rewrite'].map((p) => <option key={p} value={p}>{p}</option>)}
                    </select>
                </div>
            </div>
            {form.pass_host === 'rewrite' && (
                <div className="form-row"><label className="form-label">Upstream Host</label><input className="form-input" value={form.upstream_host} onChange={(e) => setForm({ ...form, upstream_host: e.target.value })} /></div>
            )}
            <div className="form-row">
                <label className="form-label">Backend Nodes</label>
                <div className="nodes-list">
                    {form.nodes.map((n, i) => (
                        <div key={i} className="node-row">
                            <input className="form-input" placeholder="host:port" value={n.addr} onChange={(e) => updateNode(i, 'addr', e.target.value)} />
                            <input className="form-input node-weight" type="number" min={1} placeholder="weight" value={n.weight} onChange={(e) => updateNode(i, 'weight', parseInt(e.target.value) || 1)} />
                            {form.nodes.length > 1 && <button className="node-remove" onClick={() => removeNode(i)}>✕</button>}
                        </div>
                    ))}
                    <button className="btn btn-sm" onClick={addNode} style={{ alignSelf: 'flex-start' }}>+ Add Node</button>
                </div>
            </div>
        </Modal>
    );
}
