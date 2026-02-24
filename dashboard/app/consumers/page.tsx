"use client";

import { useState } from "react";
import { useDashboard, apiPut, apiDelete, type Consumer } from "@/lib/api";
import {
  Card, Tag, Button, Modal, FormField, Input,
  SearchInput, EmptyState, useConfirm,
} from "@/components/ui";

export default function ConsumersPage() {
  const { consumers, refresh, loading } = useDashboard();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Consumer | null>(null);
  const [creating, setCreating] = useState(false);
  const { confirm, ConfirmDialog } = useConfirm();

  const [formUsername, setFormUsername] = useState("");
  const [formApiKey, setFormApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");

  const filtered = consumers.filter((c) =>
    c.username.toLowerCase().includes(search.toLowerCase()),
  );

  function openCreate() {
    setFormUsername("");
    setFormApiKey("");
    setFormError("");
    setEditing(null);
    setCreating(true);
  }

  function openEdit(c: Consumer) {
    setFormUsername(c.username);
    const keyPlugin = c.plugins?.["key-auth"] as { key?: string } | undefined;
    setFormApiKey(keyPlugin?.key ?? "");
    setFormError("");
    setCreating(false);
    setEditing(c);
  }

  function closeModal() {
    setCreating(false);
    setEditing(null);
  }

  async function handleSave() {
    if (!formUsername) {
      setFormError("Username is required");
      return;
    }
    setSaving(true);
    setFormError("");
    const body: Record<string, unknown> = { username: formUsername };
    if (formApiKey) {
      body.plugins = { "key-auth": { key: formApiKey } };
    }
    const res = await apiPut(`/consumers/${formUsername}`, body);
    setSaving(false);
    if (res.ok) {
      closeModal();
      await refresh();
    } else {
      setFormError(res.error ?? "Save failed");
    }
  }

  async function handleDelete(username: string) {
    const ok = await confirm("Delete Consumer", `Delete consumer "${username}"? This cannot be undone.`);
    if (!ok) return;
    await apiDelete(`/consumers/${username}`);
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
          <SearchInput value={search} onChange={setSearch} placeholder="Search consumers…" />
        </div>
        <div className="flex-1" />
        <Button onClick={openCreate}>+ Create Consumer</Button>
      </div>

      <Card>
        {filtered.length === 0 ? (
          <EmptyState message={search ? "No matching consumers" : "No consumers yet — click \"+ Create Consumer\" to register your first API client."} />
        ) : (
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-zinc-800 text-left text-[10px] font-semibold uppercase tracking-widest text-zinc-500">
                <th className="pb-2 pr-3">Username</th>
                <th className="pb-2 pr-3">Plugins</th>
                <th className="pb-2 pr-3">Created</th>
                <th className="pb-2" />
              </tr>
            </thead>
            <tbody>
              {filtered.map((c) => (
                <tr key={c.username} className="group border-b border-zinc-800/40 hover:bg-white/[0.02]">
                  <td className="py-2 pr-3 font-mono text-zinc-300">{c.username}</td>
                  <td className="py-2 pr-3">
                    {c.plugins ? (
                      <div className="flex flex-wrap gap-1">
                        {Object.keys(c.plugins).map((p) => (
                          <Tag key={p} color="zinc">{p}</Tag>
                        ))}
                      </div>
                    ) : (
                      <span className="text-zinc-600">—</span>
                    )}
                  </td>
                  <td className="py-2 pr-3 text-zinc-500">
                    {c.create_time
                      ? new Date(c.create_time * 1000).toLocaleDateString()
                      : "—"}
                  </td>
                  <td className="py-2 text-right">
                    <div className="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                      <Button variant="ghost" size="sm" onClick={() => openEdit(c)}>Edit</Button>
                      <Button variant="ghost" size="sm" onClick={() => handleDelete(c.username)}>Delete</Button>
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
        title={creating ? "Create Consumer" : `Edit Consumer: ${editing?.username}`}
        description={creating
          ? "A consumer represents an API client or application. Assign an API key so routes with key-auth can identify and authorize requests."
          : "Update the consumer’s credentials. Changes take effect immediately."
        }
      >
        <div className="space-y-3">
          <FormField label="Username" hint="A unique identifier for this consumer. Use the app or team name — e.g. mobile-app, billing-service.">
            <Input
              value={formUsername}
              onChange={(e) => setFormUsername(e.target.value)}
              placeholder="e.g. mobile-app"
              disabled={!!editing}
            />
          </FormField>
          <FormField label="API Key (key-auth)" hint="The secret key this consumer sends via the apikey header or query param. Leave empty for no key-auth.">
            <Input
              value={formApiKey}
              onChange={(e) => setFormApiKey(e.target.value)}
              placeholder="e.g. sk-a1b2c3d4e5f6"
            />
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
