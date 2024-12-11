'use client';

import { useEffect, useState, useCallback } from 'react';
import { services as api, Service, ListResponse } from '@/lib/api';
import Modal from '@/components/Modal';

export default function ServicesPage() {
    const [data, setData] = useState<ListResponse<Service> | null>(null);
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(true);
    const [showCreate, setShowCreate] = useState(false);
    const [editing, setEditing] = useState<Service | null>(null);
    const [confirmDelete, setConfirmDelete] = useState<Service | null>(null);

    const load = useCallback(() => {
        setLoading(true);
        api.list().then(setData).catch((e) => setError(e.message)).finally(() => setLoading(false));
    }, []);

    useEffect(() => { load(); }, [load]);

    return (
        <>
            <div className="page-header">
                <div>
                    <h1 className="page-title">Services</h1>
                    <p className="page-subtitle">Reusable sets of plugins and upstream configurations referenced by routes.</p>
                </div>
                <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Service</button>
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
                                <th>Upstream</th>
                                <th>Plugins</th>
                                <th>Status</th>
                                <th style={{ textAlign: 'right' }}>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {data.list.map((svc) => (
                                <tr key={svc.id}>
                                    <td>
                                        <div style={{ fontWeight: 600, marginBottom: 2 }}>{svc.name || svc.id}</div>
                                        {svc.description && <div style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>{svc.description}</div>}
                                    </td>
                                    <td>
                                        {svc.upstream_id
                                            ? <span className="badge badge-cyan">{svc.upstream_id}</span>
                                            : svc.upstream
                                                ? <span className="badge badge-indigo">inline ({Object.keys(svc.upstream.nodes).length} nodes)</span>
                                                : <span style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>—</span>
                                        }
                                    </td>
                                    <td>
                                        {Object.keys(svc.plugins).length > 0
                                            ? <div className="tag-list">{Object.keys(svc.plugins).map((p) => <span key={p} className="badge badge-indigo">{p}</span>)}</div>
                                            : <span style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>—</span>
                                        }
                                    </td>
                                    <td><span className={`badge ${svc.enable ? 'badge-green' : 'badge-red'}`}>{svc.enable ? 'Enabled' : 'Disabled'}</span></td>
                                    <td>
                                        <div className="actions-cell">
                                            <button className="btn btn-sm btn-icon" onClick={() => setEditing(svc)} title="Edit">✎</button>
                                            <button className="btn btn-sm btn-icon btn-danger" onClick={() => setConfirmDelete(svc)} title="Delete">✕</button>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            ) : (
                <div className="card empty-state">
                    <div className="empty-state-icon">◈</div>
                    <div className="empty-state-title">No services configured</div>
                    <div className="empty-state-desc">Services let you share plugin and upstream configs across routes.</div>
                    <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Service</button>
                </div>
            )}

            <ServiceFormModal open={showCreate} onClose={() => setShowCreate(false)} onSaved={load} />
            {editing && <ServiceFormModal open={true} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); load(); }} initial={editing} />}
            {confirmDelete && (
                <Modal title="Delete Service" open={true} onClose={() => setConfirmDelete(null)} footer={
                    <>
                        <button className="btn" onClick={() => setConfirmDelete(null)}>Cancel</button>
                        <button className="btn btn-danger" onClick={async () => { await api.delete(confirmDelete.id); setConfirmDelete(null); load(); }}>Delete</button>
                    </>
                }>
                    <p style={{ fontSize: 13 }}>Delete service <strong>{confirmDelete.name || confirmDelete.id}</strong>? Routes referencing this service will need updating.</p>
                </Modal>
            )}
        </>
    );
}

function ServiceFormModal({ open, onClose, onSaved, initial }: { open: boolean; onClose: () => void; onSaved: () => void; initial?: Service }) {
    const isEdit = !!initial;
    const [form, setForm] = useState({ name: '', description: '', upstream_id: '', plugins: '{}', enable: true });
    const [saving, setSaving] = useState(false);
    const [err, setErr] = useState('');

    useEffect(() => {
        if (initial) {
            setForm({ name: initial.name, description: initial.description, upstream_id: initial.upstream_id || '', plugins: JSON.stringify(initial.plugins || {}, null, 2), enable: initial.enable });
        } else {
            setForm({ name: '', description: '', upstream_id: '', plugins: '{}', enable: true });
        }
    }, [initial, open]);

    const handleSave = async () => {
        setSaving(true); setErr('');
        try {
            const plugins = JSON.parse(form.plugins);
            const payload: any = { name: form.name, description: form.description, upstream_id: form.upstream_id || undefined, plugins, enable: form.enable };
            if (isEdit) await api.update(initial!.id, payload);
            else await api.create(payload);
            onSaved(); onClose();
        } catch (e: any) { setErr(e.message); }
        finally { setSaving(false); }
    };

    return (
        <Modal title={isEdit ? 'Edit Service' : 'Create Service'} open={open} onClose={onClose} footer={
            <>
                <button className="btn" onClick={onClose}>Cancel</button>
                <button className="btn btn-primary" onClick={handleSave} disabled={saving}>{saving ? 'Saving…' : isEdit ? 'Update' : 'Create'}</button>
            </>
        }>
            {err && <div className="error-banner">{err}</div>}
            <div className="form-row"><label className="form-label">Name</label><input className="form-input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} /></div>
            <div className="form-row"><label className="form-label">Description</label><input className="form-input" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} /></div>
            <div className="form-row"><label className="form-label">Upstream ID</label><input className="form-input" placeholder="Optional" value={form.upstream_id} onChange={(e) => setForm({ ...form, upstream_id: e.target.value })} /></div>
            <div className="form-row"><label className="form-label">Plugins (JSON)</label><textarea className="form-input form-textarea" value={form.plugins} onChange={(e) => setForm({ ...form, plugins: e.target.value })} /></div>
            <label className="form-checkbox"><input type="checkbox" checked={form.enable} onChange={(e) => setForm({ ...form, enable: e.target.checked })} /> Enabled</label>
        </Modal>
    );
}
