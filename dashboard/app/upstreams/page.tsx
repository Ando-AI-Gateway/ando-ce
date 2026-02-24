"use client";

import { useState } from "react";
import { useDashboard, apiPut, apiDelete, type Upstream } from "@/lib/api";
import {
  Card, Tag, Button, Modal, FormField, Input, Select,
  SearchInput, EmptyState, useConfirm,
} from "@/components/ui";

export default function UpstreamsPage() {
  const { upstreams, refresh, loading } = useDashboard();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Upstream | null>(null);
  const [creating, setCreating] = useState(false);
  const { confirm, ConfirmDialog } = useConfirm();

  const [formId, setFormId] = useState("");
  const [formName, setFormName] = useState("");
  const [formHost, setFormHost] = useState("");
  const [formType, setFormType] = useState("roundrobin");
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");

  const filtered = upstreams.filter(
    (u) =>
      (u.id ?? "").toLowerCase().includes(search.toLowerCase()) ||
      (u.name ?? "").toLowerCase().includes(search.toLowerCase()),
  );

  function openCreate() {
    setFormId("");
    setFormName("");
    setFormHost("");
    setFormType("roundrobin");
    setFormError("");
    setEditing(null);
    setCreating(true);
  }

  function openEdit(u: Upstream) {
    setFormId(u.id ?? "");
    setFormName(u.name ?? "");
    const hostStr = Object.entries(u.nodes)
      .map(([addr, w]) => `${addr}:${w}`)
      .join(", ");
    setFormHost(hostStr);
    setFormType(u.type ?? "roundrobin");
    setFormError("");
    setCreating(false);
    setEditing(u);
  }

  function closeModal() {
    setCreating(false);
    setEditing(null);
  }

  async function handleSave() {
    const id = formId || editing?.id;
    if (!id || !formHost) {
      setFormError("ID and at least one node are required");
      return;
    }
    setSaving(true);
    setFormError("");
    // Parse: "host:port:weight, …" or "host:port, …"
    const nodes: Record<string, number> = {};
    for (const part of formHost.split(",")) {
      const t = part.trim();
      if (!t) continue;
      const segs = t.split(":");
      if (segs.length === 3) {
        nodes[`${segs[0]}:${segs[1]}`] = Number(segs[2]) || 1;
      } else if (segs.length === 2) {
        nodes[t] = 1;
      }
    }
    const body: Record<string, unknown> = { nodes, type: formType };
    if (formName) body.name = formName;
    const res = await apiPut(`/upstreams/${id}`, body);
    setSaving(false);
    if (res.ok) {
      closeModal();
      await refresh();
    } else {
      setFormError(res.error ?? "Save failed");
    }
  }

  async function handleDelete(id: string) {
    const ok = await confirm("Delete Upstream", `Delete upstream "${id}"? This cannot be undone.`);
    if (!ok) return;
    await apiDelete(`/upstreams/${id}`);
    await refresh();
  }

  const modalOpen = creating || !!editing;

  if (loading) {
    return <div className="flex h-48 items-center justify-center text-sm text-zinc-500">Loading…</div>;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <div className="w-64">
          <SearchInput value={search} onChange={setSearch} placeholder="Search upstreams…" />
        </div>
        <div className="flex-1" />
        <Button onClick={openCreate}>+ Create Upstream</Button>
      </div>

      <Card>
        {filtered.length === 0 ? (
          <EmptyState message={search ? "No matching upstreams" : "No upstreams yet — click \"+ Create Upstream\" to define your first backend."} />
        ) : (
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-zinc-800 text-left text-[10px] font-semibold uppercase tracking-widest text-zinc-500">
                <th className="pb-2 pr-3">ID</th>
                <th className="pb-2 pr-3">Name</th>
                <th className="pb-2 pr-3">Nodes</th>
                <th className="pb-2 pr-3">Load Balance</th>
                <th className="pb-2" />
              </tr>
            </thead>
            <tbody>
              {filtered.map((u) => (
                <tr key={u.id} className="group border-b border-zinc-800/40 hover:bg-white/[0.02]">
                  <td className="py-2 pr-3 font-mono text-zinc-300">{u.id}</td>
                  <td className="py-2 pr-3 text-zinc-400">{u.name ?? "—"}</td>
                  <td className="py-2 pr-3">
                    <div className="flex flex-wrap gap-1">
                      {Object.entries(u.nodes).map(([addr, w]) => (
                        <Tag key={addr} color="zinc">
                          {addr} w:{w}
                        </Tag>
                      ))}
                    </div>
                  </td>
                  <td className="py-2 pr-3">
                    <Tag color="blue">{u.type ?? "roundrobin"}</Tag>
                  </td>
                  <td className="py-2 text-right">
                    <div className="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                      <Button variant="ghost" size="sm" onClick={() => openEdit(u)}>Edit</Button>
                      <Button variant="ghost" size="sm" onClick={() => handleDelete(u.id!)}>Delete</Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      <Modal
        open={modalOpen}
        onClose={closeModal}
        title={creating ? "Create Upstream" : `Edit Upstream: ${editing?.id}`}
        description={creating
          ? "An upstream is a group of backend servers that routes forward traffic to. Define one or more nodes with weights for load balancing."
          : "Update the upstream nodes or load-balancing strategy. Changes take effect immediately."
        }
      >
        <div className="space-y-3">
          <FormField label="Upstream ID" hint="A unique identifier. Use lowercase with hyphens — e.g. users-backend, payment-cluster.">
            <Input value={formId} onChange={(e) => setFormId(e.target.value)} placeholder="e.g. users-backend" disabled={!!editing} />
          </FormField>
          <FormField label="Name (optional)" hint="A human-friendly label shown in the dashboard.">
            <Input value={formName} onChange={(e) => setFormName(e.target.value)} placeholder="e.g. Users Backend" />
          </FormField>
          <FormField label="Nodes" hint="Comma-separated list of host:port:weight. Weight defaults to 1 if omitted. Example: 10.0.1.5:8080:3, 10.0.1.6:8080:1">
            <Input value={formHost} onChange={(e) => setFormHost(e.target.value)} placeholder="e.g. 127.0.0.1:8080:1, 127.0.0.1:8081:2" />
          </FormField>
          <FormField label="Load Balance" hint="Round Robin distributes evenly. Consistent Hash pins clients to the same node. EWMA routes to the fastest node.">
            <Select value={formType} onChange={(e) => setFormType(e.target.value)}>
              <option value="roundrobin">Round Robin</option>
              <option value="chash">Consistent Hash</option>
              <option value="ewma">EWMA</option>
            </Select>
          </FormField>
          {formError && (
            <div className="rounded-md bg-red-500/10 px-3 py-2 text-xs text-red-400">{formError}</div>
          )}
          <div className="flex justify-end gap-2 pt-2">
            <Button variant="secondary" onClick={closeModal}>Cancel</Button>
            <Button onClick={handleSave} disabled={saving}>
              {saving ? "Saving…" : creating ? "Create" : "Save"}
            </Button>
          </div>
        </div>
      </Modal>

      <ConfirmDialog />
    </div>
  );
}
