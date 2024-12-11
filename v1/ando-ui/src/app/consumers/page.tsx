'use client';

import { useEffect, useState, useCallback } from 'react';
import { consumers as api, Consumer, ListResponse } from '@/lib/api';
import Modal from '@/components/Modal';

export default function ConsumersPage() {
    const [data, setData] = useState<ListResponse<Consumer> | null>(null);
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(true);
    const [showCreate, setShowCreate] = useState(false);
    const [detail, setDetail] = useState<Consumer | null>(null);
    const [confirmDelete, setConfirmDelete] = useState<Consumer | null>(null);

    const load = useCallback(() => {
        setLoading(true);
        api.list().then(setData).catch((e) => setError(e.message)).finally(() => setLoading(false));
    }, []);

    useEffect(() => { load(); }, [load]);

    return (
        <>
            <div className="page-header">
                <div>
                    <h1 className="page-title">Consumers</h1>
                    <p className="page-subtitle">API clients with authentication credentials and per-consumer plugin configs.</p>
                </div>
                <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Consumer</button>
            </div>

            {error && <div className="error-banner">⚠ {error}</div>}

            {loading ? (
                <div className="loading-spinner"><div className="spinner" /></div>
            ) : data && data.list.length > 0 ? (
                <div className="table-wrapper">
                    <table className="table">
                        <thead>
                            <tr>
                                <th>Username</th>
                                <th>Group</th>
                                <th>Plugins</th>
                                <th>Created</th>
                                <th style={{ textAlign: 'right' }}>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {data.list.map((c) => (
                                <tr key={c.id}>
                                    <td>
                                        <div style={{ fontWeight: 600 }}>{c.username}</div>
                                        {c.description && <div style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>{c.description}</div>}
                                    </td>
                                    <td>{c.group ? <span className="badge badge-cyan">{c.group}</span> : <span style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>—</span>}</td>
                                    <td>
                                        <div className="tag-list">
                                            {Object.keys(c.plugins).map((p) => <span key={p} className="badge badge-indigo">{p}</span>)}
                                            {Object.keys(c.plugins).length === 0 && <span style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>—</span>}
                                        </div>
                                    </td>
                                    <td style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{c.created_at ? new Date(c.created_at).toLocaleDateString() : '—'}</td>
                                    <td>
                                        <div className="actions-cell">
                                            <button className="btn btn-sm btn-icon" onClick={() => setDetail(c)} title="View">◎</button>
                                            <button className="btn btn-sm btn-icon btn-danger" onClick={() => setConfirmDelete(c)} title="Delete">✕</button>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            ) : (
                <div className="card empty-state">
                    <div className="empty-state-icon">⊕</div>
                    <div className="empty-state-title">No consumers</div>
                    <div className="empty-state-desc">Consumers represent API clients with authentication credentials.</div>
                    <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Consumer</button>
                </div>
            )}

            <ConsumerFormModal open={showCreate} onClose={() => setShowCreate(false)} onSaved={load} />

            {detail && (
                <Modal title={`Consumer: ${detail.username}`} open={true} onClose={() => setDetail(null)}>
                    <div className="form-row"><label className="form-label">ID</label><div className="code-inline" style={{ display: 'inline' }}>{detail.id}</div></div>
                    <div className="form-row"><label className="form-label">Username</label><div>{detail.username}</div></div>
                    {detail.description && <div className="form-row"><label className="form-label">Description</label><div>{detail.description}</div></div>}
                    {detail.group && <div className="form-row"><label className="form-label">Group</label><span className="badge badge-cyan">{detail.group}</span></div>}
                    <div className="form-row">
                        <label className="form-label">Plugin Credentials</label>
                        <div className="json-display">{JSON.stringify(detail.plugins, null, 2)}</div>
                    </div>
                </Modal>
            )}

            {confirmDelete && (
                <Modal title="Delete Consumer" open={true} onClose={() => setConfirmDelete(null)} footer={
                    <>
                        <button className="btn" onClick={() => setConfirmDelete(null)}>Cancel</button>
                        <button className="btn btn-danger" onClick={async () => { await api.delete(confirmDelete.id); setConfirmDelete(null); load(); }}>Delete</button>
                    </>
                }>
                    <p style={{ fontSize: 13 }}>Delete consumer <strong>{confirmDelete.username}</strong>? Their API credentials will stop working immediately.</p>
                </Modal>
            )}
        </>
    );
}

function ConsumerFormModal({ open, onClose, onSaved }: { open: boolean; onClose: () => void; onSaved: () => void }) {
    const [form, setForm] = useState({ username: '', description: '', group: '', plugins: '{}' });
    const [saving, setSaving] = useState(false);
    const [err, setErr] = useState('');

    useEffect(() => {
        if (open) setForm({ username: '', description: '', group: '', plugins: '{}' });
    }, [open]);

    const handleSave = async () => {
        setSaving(true); setErr('');
        try {
            const plugins = JSON.parse(form.plugins);
            await api.create({ username: form.username, description: form.description, group: form.group || undefined, plugins });
            onSaved(); onClose();
        } catch (e: any) { setErr(e.message); }
        finally { setSaving(false); }
    };

    return (
        <Modal title="Create Consumer" open={open} onClose={onClose} footer={
            <>
                <button className="btn" onClick={onClose}>Cancel</button>
                <button className="btn btn-primary" onClick={handleSave} disabled={saving}>{saving ? 'Saving…' : 'Create'}</button>
            </>
        }>
            {err && <div className="error-banner">{err}</div>}
            <div className="form-row"><label className="form-label">Username *</label><input className="form-input" value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value })} /></div>
            <div className="form-row"><label className="form-label">Description</label><input className="form-input" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} /></div>
            <div className="form-row"><label className="form-label">Group</label><input className="form-input" placeholder="Optional" value={form.group} onChange={(e) => setForm({ ...form, group: e.target.value })} /></div>
            <div className="form-row">
                <label className="form-label">Plugin Credentials (JSON)</label>
                <textarea className="form-input form-textarea" value={form.plugins} onChange={(e) => setForm({ ...form, plugins: e.target.value })} />
                <div className="form-hint">Example: {`{"key-auth": {"key": "my-secret-key"}}`}</div>
            </div>
        </Modal>
    );
}
