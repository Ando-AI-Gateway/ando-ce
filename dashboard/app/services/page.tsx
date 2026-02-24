"use client";

import { useState } from "react";
import { useDashboard, apiPut, apiDelete, type Service } from "@/lib/api";
import {
  Card, Tag, Button, Modal, FormField, Input, Select,
  SearchInput, EmptyState, useConfirm,
} from "@/components/ui";

export default function ServicesPage() {
  const { services, upstreams, refresh, loading } = useDashboard();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Service | null>(null);
  const [creating, setCreating] = useState(false);
  const { confirm, ConfirmDialog } = useConfirm();

  // Form state
  const [formId, setFormId] = useState("");
  const [formName, setFormName] = useState("");
  const [formDesc, setFormDesc] = useState("");
  const [formUpstream, setFormUpstream] = useState("");   // named upstream_id
  const [formInlineNodes, setFormInlineNodes] = useState(""); // inline host:port
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");

  const filtered = services.filter(
    (s) =>
      s.id.toLowerCase().includes(search.toLowerCase()) ||
      (s.name ?? "").toLowerCase().includes(search.toLowerCase()),
  );

  function openCreate() {
    setFormId("");
    setFormName("");
    setFormDesc("");
    setFormUpstream("");
    setFormInlineNodes("");
    setFormError("");
    setEditing(null);
    setCreating(true);
  }

  function openEdit(s: Service) {
    setFormId(s.id);
    setFormName(s.name ?? "");
    setFormDesc(s.desc ?? "");
    setFormUpstream(s.upstream_id ?? "");
    const inlineNodes = s.upstream_id
      ? ""
      : Object.entries(s.upstream?.nodes ?? {})
          .map(([addr, w]) => (w === 1 ? addr : `${addr}:${w}`))
          .join(", ");
    setFormInlineNodes(inlineNodes);
    setFormError("");
    setCreating(false);
    setEditing(s);
  }

  function closeModal() {
    setCreating(false);
    setEditing(null);
  }

  async function handleSave() {
    const id = formId || editing?.id;
    if (!id) {
      setFormError("Service ID is required");
      return;
    }
    if (!formUpstream && !formInlineNodes.trim()) {
      setFormError("An upstream or at least one inline node is required");
      return;
    }
    setSaving(true);
    setFormError("");

    const body: Record<string, unknown> = {};
    if (formName) body.name = formName;
    if (formDesc) body.desc = formDesc;

    if (formUpstream) {
      body.upstream_id = formUpstream;
    } else {
      const nodes: Record<string, number> = {};
      formInlineNodes.split(",").forEach((s) => {
        const t = s.trim();
        if (!t) return;
        const lastColon = t.lastIndexOf(":");
        if (lastColon > 0) {
          const maybeWeight = Number(t.slice(lastColon + 1));
          if (!isNaN(maybeWeight) && maybeWeight > 0 && t.slice(lastColon + 1) !== "") {
            nodes[t.slice(0, lastColon)] = maybeWeight;
          } else {
            nodes[t] = 1;
          }
        } else {
          nodes[t] = 1;
        }
      });
      if (Object.keys(nodes).length > 0) body.upstream = { nodes };
    }

    const res = await apiPut(`/services/${id}`, body);
    setSaving(false);
    if (res.ok) {
      closeModal();
      await refresh();
    } else {
      setFormError(res.error ?? "Save failed");
    }
  }

  async function handleDelete(id: string) {
    const ok = await confirm("Delete Service", `Delete service "${id}"? Routes referencing this service will lose their upstream.`);
    if (!ok) return;
    await apiDelete(`/services/${id}`);
    await refresh();
  }

  const modalOpen = creating || !!editing;

  if (loading) {
    return (
      <div className="flex h-48 items-center justify-center text-sm text-zinc-500">
        Loading…
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* APISIX hierarchy info banner */}
      <div className="flex items-start gap-3 rounded-xl border border-zinc-800 bg-zinc-900/60 px-4 py-3 text-xs text-zinc-500">
        <svg className="mt-0.5 h-3.5 w-3.5 shrink-0 text-zinc-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="12" cy="12" r="10" /><path d="M12 8v4M12 16h.01" />
        </svg>
        <span>
          <span className="font-semibold text-zinc-400">Service</span> bundles an upstream with optional plugins.
          Multiple routes can share one service — just set <span className="font-mono text-zinc-400">service_id</span> on a route.
          {" "}Hierarchy:{" "}
          <span className="font-semibold text-zinc-300">Route → Service → Upstream</span>
        </span>
      </div>

      <div className="flex items-center gap-3">
        <div className="w-64">
          <SearchInput value={search} onChange={setSearch} placeholder="Search services…" />
        </div>
        <div className="flex-1" />
        <Button onClick={openCreate}>+ Create Service</Button>
      </div>

      <Card>
        {filtered.length === 0 ? (
          <EmptyState message={search ? "No matching services" : "No services yet — click \"+ Create Service\" to add your first one."} />
        ) : (
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-zinc-800 text-left text-[10px] font-semibold uppercase tracking-widest text-zinc-500">
                <th className="pb-2 pr-3">ID</th>
                <th className="pb-2 pr-3">Name</th>
                <th className="pb-2 pr-3">Upstream</th>
                <th className="pb-2 pr-3">Plugins</th>
                <th className="pb-2" />
              </tr>
            </thead>
            <tbody>
              {filtered.map((s) => (
                <tr key={s.id} className="group border-b border-zinc-800/40 hover:bg-white/[0.02]">
                  <td className="py-2 pr-3 font-mono text-zinc-300">{s.id}</td>
                  <td className="py-2 pr-3 text-zinc-400">{s.name ?? "—"}</td>
                  <td className="py-2 pr-3 font-mono text-zinc-500">
                    {s.upstream_id ? (
                      <Tag color="blue">{s.upstream_id}</Tag>
                    ) : s.upstream?.nodes ? (
                      <span className="text-zinc-600">
                        {Object.keys(s.upstream.nodes).join(", ")}
                      </span>
                    ) : (
                      <span className="text-zinc-700">—</span>
                    )}
                  </td>
                  <td className="py-2 pr-3">
                    {s.plugins && Object.keys(s.plugins).length > 0 ? (
                      <div className="flex flex-wrap gap-1">
                        {Object.keys(s.plugins).map((p) => (
                          <Tag key={p} color="zinc">{p}</Tag>
                        ))}
                      </div>
                    ) : (
                      <span className="text-zinc-600">—</span>
                    )}
                  </td>
                  <td className="py-2 text-right">
                    <div className="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                      <Button variant="ghost" size="sm" onClick={() => openEdit(s)}>
                        Edit
                      </Button>
                      <Button variant="ghost" size="sm" onClick={() => handleDelete(s.id)}>
                        Delete
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      {/* Create / Edit modal */}
      <Modal
        open={modalOpen}
        onClose={closeModal}
        title={creating ? "Create Service" : `Edit Service: ${editing?.id}`}
        description={creating
          ? "A service bundles an upstream with shared plugin configuration. Attach multiple routes to one service to avoid repeating upstream settings."
          : "Update the service. Routes using this service will pick up changes immediately."
        }
      >
        <div className="space-y-3">
          <FormField label="Service ID" hint="Unique identifier — e.g. users-service, payment-svc.">
            <Input
              value={formId}
              onChange={(e) => setFormId(e.target.value)}
              placeholder="e.g. users-service"
              disabled={!!editing}
            />
          </FormField>
          <FormField label="Name (optional)" hint="Human-friendly label shown in tables and dropdowns.">
            <Input value={formName} onChange={(e) => setFormName(e.target.value)} placeholder="e.g. Users Service" />
          </FormField>
          <FormField label="Description (optional)">
            <Input value={formDesc} onChange={(e) => setFormDesc(e.target.value)} placeholder="e.g. Handles /api/users routes" />
          </FormField>
          <FormField label="Upstream" hint="Select a named upstream, or leave as None and enter inline nodes below.">
            <Select value={formUpstream} onChange={(e) => setFormUpstream(e.target.value)}>
              <option value="">None (inline)</option>
              {upstreams.map((u) => (
                <option key={u.id} value={u.id}>
                  {u.name ? `${u.name} (${u.id})` : u.id}
                </option>
              ))}
            </Select>
          </FormField>
          {!formUpstream && (
            <FormField
              label="Inline nodes"
              hint="host:port or host:port:weight — e.g. 127.0.0.1:8080 or 10.0.0.1:8080:10. Multiple nodes: comma-separated."
            >
              <Input
                value={formInlineNodes}
                onChange={(e) => setFormInlineNodes(e.target.value)}
                placeholder="e.g. 127.0.0.1:8080"
              />
            </FormField>
          )}
          {formError && (
            <div className="rounded-md bg-red-500/10 px-3 py-2 text-xs text-red-400">
              {formError}
            </div>
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
