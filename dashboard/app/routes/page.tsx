"use client";

import { useState, useEffect, useRef } from "react";
import { useDashboard, apiPut, apiDelete, type Route } from "@/lib/api";
import {
  Card, Tag, Button, Modal, FormField, Input, Select,
  SearchInput, EmptyState, useConfirm,
} from "@/components/ui";

// ── Test Request Modal ───────────────────────────────────────────────────────

interface TestHeader { key: string; value: string; id: number }

interface TestResponse {
  status: number;
  statusText: string;
  headers: Record<string, string>;
  body: string;
  durationMs: number;
}

function TestRequestModal({
  route,
  open,
  onClose,
}: {
  route: Route | null;
  open: boolean;
  onClose: () => void;
}) {
  const defaultPath = route
    ? route.uri.replace(/[*{].*$/, "").replace(/\/$/, "") || "/"
    : "/";
  const defaultMethod =
    route?.methods && route.methods.length > 0 ? route.methods[0] : "GET";

  const [proxyBase, setProxyBase] = useState("http://localhost:9080");
  const [editingBase, setEditingBase] = useState(false);
  const baseInputRef = useRef<HTMLInputElement>(null);
  const [method, setMethod] = useState(defaultMethod);
  const [path, setPath] = useState(defaultPath);

  // Auto-detect proxy host from current page hostname
  useEffect(() => {
    if (typeof window !== "undefined") {
      setProxyBase(`http://${window.location.hostname}:9080`);
    }
  }, []);
  const [headers, setHeaders] = useState<TestHeader[]>([
    { key: "Content-Type", value: "application/json", id: Date.now() },
  ]);
  const [body, setBody] = useState("");
  const [sending, setSending] = useState(false);
  const [response, setResponse] = useState<TestResponse | null>(null);
  const [reqError, setReqError] = useState("");

  // Sync form when route changes
  function reset(r: Route | null) {
    const p = r ? r.uri.replace(/[*{].*$/, "").replace(/\/$/, "") || "/" : "/";
    const m = r?.methods && r.methods.length > 0 ? r.methods[0] : "GET";
    setMethod(m);
    setPath(p);
    setBody("");
    setResponse(null);
    setReqError("");
  }

  function addHeader() {
    setHeaders((h) => [...h, { key: "", value: "", id: Date.now() }]);
  }

  function removeHeader(id: number) {
    setHeaders((h) => h.filter((x) => x.id !== id));
  }

  function updateHeader(id: number, field: "key" | "value", val: string) {
    setHeaders((h) => h.map((x) => (x.id === id ? { ...x, [field]: val } : x)));
  }

  async function sendRequest() {
    setSending(true);
    setReqError("");
    setResponse(null);

    const url = `${proxyBase.replace(/\/$/, "")}${path}`;
    const reqHeaders: Record<string, string> = {};
    for (const h of headers) {
      if (h.key.trim()) reqHeaders[h.key.trim()] = h.value;
    }

    const t0 = performance.now();
    try {
      const res = await fetch(url, {
        method,
        headers: reqHeaders,
        body: ["GET", "HEAD", "DELETE"].includes(method) ? undefined : body || undefined,
      });
      const durationMs = Math.round(performance.now() - t0);
      const resHeaders: Record<string, string> = {};
      res.headers.forEach((v, k) => { resHeaders[k] = v; });
      const text = await res.text();
      setResponse({ status: res.status, statusText: res.statusText, headers: resHeaders, body: text, durationMs });
    } catch (e) {
      const durationMs = Math.round(performance.now() - t0);
      setReqError(`${e instanceof Error ? e.message : String(e)} (${durationMs}ms)`);
    } finally {
      setSending(false);
    }
  }

  const hasBody = !["GET", "HEAD", "DELETE"].includes(method);
  const statusColor =
    !response ? "" :
    response.status < 300 ? "text-green-400" :
    response.status < 400 ? "text-yellow-400" : "text-red-400";

  return (
    <Modal
      open={open}
      onClose={() => { reset(route); onClose(); }}
      title={`Test Route: ${route?.id ?? ""}`}
    >
      <div className="space-y-3">
        {/* Method + path row */}
        <div className="flex gap-2">
          <div className="w-28">
            <Select value={method} onChange={(e) => setMethod(e.target.value)}>
              {["GET","POST","PUT","PATCH","DELETE","HEAD","OPTIONS"].map((m) => (
                <option key={m}>{m}</option>
              ))}
            </Select>
          </div>
          <div className="flex-1">
            <Input
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="/api/v1/resource"
            />
          </div>
        </div>

        {/* Full URL preview + proxy edit */}
        <div className="rounded-md bg-zinc-900 px-3 py-1.5 font-mono text-[10px] text-zinc-500 break-all flex items-center gap-1.5">
          <span className="flex-1 break-all">{proxyBase.replace(/\/$/, "")}{path}</span>
          {editingBase ? (
            <input
              ref={baseInputRef}
              value={proxyBase}
              onChange={(e) => setProxyBase(e.target.value)}
              onBlur={() => setEditingBase(false)}
              onKeyDown={(e) => { if (e.key === "Enter" || e.key === "Escape") setEditingBase(false); }}
              className="ml-1 w-52 rounded border border-zinc-600 bg-zinc-800 px-1.5 py-0.5 font-mono text-[10px] text-zinc-300 outline-none focus:border-zinc-400"
              autoFocus
            />
          ) : (
            <button
              onClick={() => { setEditingBase(true); setTimeout(() => baseInputRef.current?.select(), 0); }}
              className="ml-1 shrink-0 text-[10px] text-zinc-600 hover:text-zinc-300"
              title="Change proxy URL"
            >
              change proxy
            </button>
          )}
        </div>

        {/* Request headers */}
        <div>
          <div className="mb-1 flex items-center justify-between">
            <span className="text-[11px] font-medium text-zinc-400">Headers</span>
            <button
              onClick={addHeader}
              className="text-[10px] text-zinc-500 hover:text-zinc-300"
            >
              + Add
            </button>
          </div>
          <div className="space-y-1.5">
            {headers.map((h) => (
              <div key={h.id} className="flex gap-1.5">
                <Input
                  value={h.key}
                  onChange={(e) => updateHeader(h.id, "key", e.target.value)}
                  placeholder="Key"
                />
                <Input
                  value={h.value}
                  onChange={(e) => updateHeader(h.id, "value", e.target.value)}
                  placeholder="Value"
                />
                <button
                  onClick={() => removeHeader(h.id)}
                  className="px-1.5 text-zinc-600 hover:text-red-400"
                >
                  ×
                </button>
              </div>
            ))}
          </div>
        </div>

        {/* Body */}
        {hasBody && (
          <FormField label="Body (JSON / text)">
            <textarea
              className="w-full rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 font-mono text-xs text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-500 resize-y"
              rows={4}
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder='{"key": "value"}'
            />
          </FormField>
        )}

        {/* Send button */}
        <div className="flex items-center justify-between pt-1">
          <span className="text-[10px] text-zinc-600">
            Request is made directly from your browser
          </span>
          <Button onClick={sendRequest} disabled={sending}>
            {sending ? "Sending…" : "Send Request"}
          </Button>
        </div>

        {/* Error */}
        {reqError && (
          <div className="rounded-md bg-red-500/10 px-3 py-2 text-xs text-red-400 font-mono break-all">
            {reqError}
          </div>
        )}

        {/* Response */}
        {response && (
          <div className="rounded-md border border-zinc-700 bg-zinc-900">
            {/* Status bar */}
            <div className="flex items-center gap-3 border-b border-zinc-800 px-3 py-2">
              <span className={`font-mono text-sm font-semibold ${statusColor}`}>
                {response.status} {response.statusText}
              </span>
              <span className="text-[10px] text-zinc-600">{response.durationMs}ms</span>
            </div>
            {/* Response headers */}
            {Object.keys(response.headers).length > 0 && (
              <div className="border-b border-zinc-800 px-3 py-2">
                <div className="mb-1 text-[10px] font-semibold uppercase tracking-widest text-zinc-600">
                  Response Headers
                </div>
                <div className="space-y-0.5 font-mono text-[10px]">
                  {Object.entries(response.headers).map(([k, v]) => (
                    <div key={k} className="flex gap-2">
                      <span className="text-zinc-500 min-w-0 shrink-0">{k}:</span>
                      <span className="text-zinc-300 break-all">{v}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
            {/* Response body */}
            <div className="px-3 py-2">
              <div className="mb-1 text-[10px] font-semibold uppercase tracking-widest text-zinc-600">
                Body
              </div>
              <pre className="max-h-56 overflow-auto font-mono text-[11px] text-zinc-300 whitespace-pre-wrap break-all">
                {(() => {
                  try {
                    return JSON.stringify(JSON.parse(response.body), null, 2);
                  } catch {
                    return response.body || "(empty)";
                  }
                })()}
              </pre>
            </div>
          </div>
        )}

        <div className="flex justify-end pt-1">
          <Button variant="secondary" onClick={() => { reset(route); onClose(); }}>
            Close
          </Button>
        </div>
      </div>
    </Modal>
  );
}

// ── Routes Page ──────────────────────────────────────────────────────────────

export default function RoutesPage() {
  const { routes, upstreams, refresh, loading } = useDashboard();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Route | null>(null);
  const [creating, setCreating] = useState(false);
  const [testing, setTesting] = useState<Route | null>(null);
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
                      <Button variant="ghost" size="sm" onClick={() => setTesting(r)}>
                        Test
                      </Button>
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
          <div className="flex justify-between gap-2 pt-2">
            <Button
              variant="secondary"
              onClick={() => {
                const route = editing ?? {
                  id: formId || "preview",
                  uri: formUri || "/",
                  methods: formMethods.split(",").map((m) => m.trim().toUpperCase()).filter(Boolean),
                  upstream_id: formUpstream || undefined,
                  status: 1,
                };
                setTesting(route as Route);
              }}
              disabled={!formUri}
            >
              Test Request
            </Button>
            <div className="flex gap-2">
              <Button variant="secondary" onClick={closeModal}>
                Cancel
              </Button>
              <Button onClick={handleSave} disabled={saving}>
                {saving ? "Saving…" : creating ? "Create" : "Save"}
              </Button>
            </div>
          </div>
        </div>
      </Modal>

      <TestRequestModal
        route={testing}
        open={!!testing}
        onClose={() => setTesting(null)}
      />

      <ConfirmDialog />
    </div>
  );
}
