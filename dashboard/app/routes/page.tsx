"use client";

import { useState } from "react";
import { useDashboard, apiPut, apiDelete, type Route } from "@/lib/api";
import {
  Card, Tag, Button, Modal, FormField, Input, Select,
  SearchInput, EmptyState, useConfirm,
} from "@/components/ui";

export default function RoutesPage() {
  const { routes, upstreams, refresh, loading } = useDashboard();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Route | null>(null);
  const [creating, setCreating] = useState(false);
  const { confirm, ConfirmDialog } = useConfirm();

  // Form state
  const [formId, setFormId] = useState("");
  const [formName, setFormName] = useState("");
  const [formUri, setFormUri] = useState("");
  const [formMethods, setFormMethods] = useState("");
  const [formUpstream, setFormUpstream] = useState("");
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");

  const filtered = routes.filter(
    (r) =>
      r.id.toLowerCase().includes(search.toLowerCase()) ||
      r.uri.toLowerCase().includes(search.toLowerCase()) ||
      (r.name ?? "").toLowerCase().includes(search.toLowerCase()),
  );

  function openCreate() {
    setFormId("");
    setFormName("");
    setFormUri("");
    setFormMethods("GET");
    setFormUpstream("");
    setFormError("");
    setEditing(null);
    setCreating(true);
  }

  function openEdit(r: Route) {
    setFormId(r.id);
    setFormName(r.name ?? "");
    setFormUri(r.uri);
    setFormMethods((r.methods ?? []).join(", "));
    setFormUpstream(r.upstream_id ?? "");
    setFormError("");
    setCreating(false);
    setEditing(r);
  }

  function closeModal() {
    setCreating(false);
    setEditing(null);
  }

  async function handleSave() {
    if (!formId || !formUri) {
      setFormError("ID and URI are required");
      return;
    }
    setSaving(true);
    setFormError("");
    const methods = formMethods
      .split(",")
      .map((m) => m.trim().toUpperCase())
      .filter(Boolean);
    const body: Record<string, unknown> = { uri: formUri, methods };
    if (formName) body.name = formName;
    if (formUpstream) body.upstream_id = formUpstream;
    const res = await apiPut(`/routes/${formId}`, body);
    setSaving(false);
    if (res.ok) {
      closeModal();
      await refresh();
    } else {
      setFormError(res.error ?? "Save failed");
    }
  }

  async function handleDelete(id: string) {
    const ok = await confirm("Delete Route", `Delete route "${id}"? This cannot be undone.`);
    if (!ok) return;
    await apiDelete(`/routes/${id}`);
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
      <div className="flex items-center gap-3">
        <div className="w-64">
          <SearchInput value={search} onChange={setSearch} placeholder="Search routes…" />
        </div>
        <div className="flex-1" />
        <Button onClick={openCreate}>+ Create Route</Button>
      </div>

      <Card>
        {filtered.length === 0 ? (
          <EmptyState message={search ? "No matching routes" : "No routes configured"} />
        ) : (
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-zinc-800 text-left text-[10px] font-semibold uppercase tracking-widest text-zinc-500">
                <th className="pb-2 pr-3">ID</th>
                <th className="pb-2 pr-3">Name</th>
                <th className="pb-2 pr-3">Methods</th>
                <th className="pb-2 pr-3">URI</th>
                <th className="pb-2 pr-3">Upstream</th>
                <th className="pb-2 pr-3">Plugins</th>
                <th className="pb-2 pr-3">Status</th>
                <th className="pb-2" />
              </tr>
            </thead>
            <tbody>
              {filtered.map((r) => (
                <tr key={r.id} className="group border-b border-zinc-800/40 hover:bg-white/[0.02]">
                  <td className="py-2 pr-3 font-mono text-zinc-300">{r.id}</td>
                  <td className="py-2 pr-3 text-zinc-400">{r.name ?? "—"}</td>
                  <td className="py-2 pr-3">
                    <div className="flex flex-wrap gap-1">
                      {(r.methods ?? ["*"]).map((m) => (
                        <Tag key={m} color="blue">{m}</Tag>
                      ))}
                    </div>
                  </td>
                  <td className="py-2 pr-3 font-mono text-zinc-400">{r.uri}</td>
                  <td className="py-2 pr-3 font-mono text-zinc-500">{r.upstream_id ?? "inline"}</td>
                  <td className="py-2 pr-3">
                    {r.plugins ? (
                      <div className="flex flex-wrap gap-1">
                        {Object.keys(r.plugins).map((p) => (
                          <Tag key={p} color="zinc">{p}</Tag>
                        ))}
                      </div>
                    ) : (
                      <span className="text-zinc-600">—</span>
                    )}
                  </td>
                  <td className="py-2 pr-3">
                    <Tag color={r.status === 0 ? "red" : "green"}>
                      {r.status === 0 ? "Off" : "On"}
                    </Tag>
                  </td>
                  <td className="py-2 text-right">
                    <div className="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                      <Button variant="ghost" size="sm" onClick={() => openEdit(r)}>
                        Edit
                      </Button>
                      <Button variant="ghost" size="sm" onClick={() => handleDelete(r.id)}>
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
        title={creating ? "Create Route" : `Edit Route: ${editing?.id}`}
      >
        <div className="space-y-3">
          <FormField label="Route ID">
            <Input
              value={formId}
              onChange={(e) => setFormId(e.target.value)}
              placeholder="my-route"
              disabled={!!editing}
            />
          </FormField>
          <FormField label="Name (optional)">
            <Input value={formName} onChange={(e) => setFormName(e.target.value)} placeholder="My Route" />
          </FormField>
          <FormField label="URI">
            <Input value={formUri} onChange={(e) => setFormUri(e.target.value)} placeholder="/api/*" />
          </FormField>
          <FormField label="Methods (comma-separated)">
            <Input
              value={formMethods}
              onChange={(e) => setFormMethods(e.target.value)}
              placeholder="GET, POST"
            />
          </FormField>
          <FormField label="Upstream">
            <Select value={formUpstream} onChange={(e) => setFormUpstream(e.target.value)}>
              <option value="">None (inline)</option>
              {upstreams.map((u) => (
                <option key={u.id} value={u.id}>
                  {u.name ?? u.id}
                </option>
              ))}
            </Select>
          </FormField>
          {formError && (
            <div className="rounded-md bg-red-500/10 px-3 py-2 text-xs text-red-400">
              {formError}
            </div>
          )}
          <div className="flex justify-end gap-2 pt-2">
            <Button variant="secondary" onClick={closeModal}>
              Cancel
            </Button>
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
