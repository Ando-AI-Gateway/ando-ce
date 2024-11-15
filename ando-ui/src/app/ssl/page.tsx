'use client';

import { useEffect, useState, useCallback } from 'react';
import { ssl as api, SslCert, ListResponse } from '@/lib/api';
import Modal from '@/components/Modal';

export default function SslPage() {
    const [data, setData] = useState<ListResponse<SslCert> | null>(null);
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(true);
    const [showCreate, setShowCreate] = useState(false);
    const [confirmDelete, setConfirmDelete] = useState<SslCert | null>(null);

    const load = useCallback(() => {
        setLoading(true);
        api.list().then(setData).catch((e) => setError(e.message)).finally(() => setLoading(false));
    }, []);

    useEffect(() => { load(); }, [load]);

    return (
        <>
            <div className="page-header">
                <div>
                    <h1 className="page-title">SSL Certificates</h1>
                    <p className="page-subtitle">Manage TLS certificates for HTTPS and SNI-based routing.</p>
                </div>
                <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Upload Certificate</button>
            </div>

            {error && <div className="error-banner">⚠ {error}</div>}

            {loading ? (
                <div className="loading-spinner"><div className="spinner" /></div>
            ) : data && data.list.length > 0 ? (
                <div className="table-wrapper">
                    <table className="table">
                        <thead>
                            <tr>
                                <th>SNI Hostnames</th>
                                <th>Status</th>
                                <th>Expiry</th>
                                <th>mTLS</th>
                                <th style={{ textAlign: 'right' }}>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {data.list.map((cert) => (
                                <tr key={cert.id}>
                                    <td>
                                        <div className="tag-list">
                                            {cert.snis.map((sni) => <span key={sni} className="badge badge-cyan">{sni}</span>)}
                                        </div>
                                    </td>
                                    <td><span className={`badge ${cert.status ? 'badge-green' : 'badge-red'}`}>{cert.status ? 'Active' : 'Disabled'}</span></td>
                                    <td style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{cert.validity_end ? new Date(cert.validity_end).toLocaleDateString() : '—'}</td>
                                    <td>{cert.client_cert ? <span className="badge badge-yellow">Yes</span> : <span style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>No</span>}</td>
                                    <td>
                                        <div className="actions-cell">
                                            <button className="btn btn-sm btn-icon btn-danger" onClick={() => setConfirmDelete(cert)} title="Delete">✕</button>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            ) : (
                <div className="card empty-state">
                    <div className="empty-state-icon">⊘</div>
                    <div className="empty-state-title">No SSL certificates</div>
                    <div className="empty-state-desc">Upload certificates to enable HTTPS on your gateway.</div>
                    <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Upload Certificate</button>
                </div>
            )}

            <SslFormModal open={showCreate} onClose={() => setShowCreate(false)} onSaved={load} />
            {confirmDelete && (
                <Modal title="Delete Certificate" open={true} onClose={() => setConfirmDelete(null)} footer={
                    <>
                        <button className="btn" onClick={() => setConfirmDelete(null)}>Cancel</button>
                        <button className="btn btn-danger" onClick={async () => { await api.delete(confirmDelete.id); setConfirmDelete(null); load(); }}>Delete</button>
                    </>
                }>
                    <p style={{ fontSize: 13 }}>Delete certificate for <strong>{confirmDelete.snis.join(', ')}</strong>? HTTPS will stop working for these domains.</p>
                </Modal>
            )}
        </>
    );
}

function SslFormModal({ open, onClose, onSaved }: { open: boolean; onClose: () => void; onSaved: () => void }) {
    const [form, setForm] = useState({ snis: '', cert: '', key: '' });
    const [saving, setSaving] = useState(false);
    const [err, setErr] = useState('');

    useEffect(() => { if (open) setForm({ snis: '', cert: '', key: '' }); }, [open]);

    const handleSave = async () => {
        setSaving(true); setErr('');
        try {
            const snis = form.snis.split(',').map((s) => s.trim()).filter(Boolean);
            if (snis.length === 0) throw new Error('At least one SNI hostname is required');
            if (!form.cert) throw new Error('Certificate PEM is required');
            if (!form.key) throw new Error('Private key PEM is required');
            await api.create({ snis, cert: form.cert, key: form.key });
            onSaved(); onClose();
        } catch (e: any) { setErr(e.message); }
        finally { setSaving(false); }
    };

    return (
        <Modal title="Upload SSL Certificate" open={open} onClose={onClose} large footer={
            <>
                <button className="btn" onClick={onClose}>Cancel</button>
                <button className="btn btn-primary" onClick={handleSave} disabled={saving}>{saving ? 'Uploading…' : 'Upload'}</button>
            </>
        }>
            {err && <div className="error-banner">{err}</div>}
            <div className="form-row">
                <label className="form-label">SNI Hostnames *</label>
                <input className="form-input" placeholder="api.example.com, *.example.com" value={form.snis} onChange={(e) => setForm({ ...form, snis: e.target.value })} />
                <div className="form-hint">Comma-separated list of hostnames (SNI) this certificate covers.</div>
            </div>
            <div className="form-row">
                <label className="form-label">Certificate (PEM) *</label>
                <textarea className="form-input form-textarea" placeholder="-----BEGIN CERTIFICATE-----" value={form.cert} onChange={(e) => setForm({ ...form, cert: e.target.value })} style={{ minHeight: 150 }} />
            </div>
            <div className="form-row">
                <label className="form-label">Private Key (PEM) *</label>
                <textarea className="form-input form-textarea" placeholder="-----BEGIN PRIVATE KEY-----" value={form.key} onChange={(e) => setForm({ ...form, key: e.target.value })} style={{ minHeight: 150 }} />
            </div>
        </Modal>
    );
}
