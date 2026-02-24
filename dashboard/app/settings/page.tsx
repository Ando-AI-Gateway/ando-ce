"use client";

import { Card, Button, Tag } from "@/components/ui";
import { useDashboard } from "@/lib/api";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function formatUnixTime(unix: number): string {
  return new Date(unix * 1000).toLocaleString(undefined, {
    year: "numeric", month: "short", day: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit",
  });
}

function secondsAgo(unix: number): string {
  const diff = Math.floor(Date.now() / 1000 - unix);
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

export default function SettingsPage() {
  const { edition, version, healthy, persistence } = useDashboard();
  const isEnterprise = edition === "enterprise";
  const hasPersistence = persistence.mode === "file";
  const isFileHealthy = hasPersistence && persistence.file_exists;

  return (
    <div className="space-y-6">
      {/* Connection */}
      <Card title="Connection">
        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-[11px] font-semibold uppercase tracking-widest text-zinc-500">
              Admin API Listen Address
            </label>
            <div className="flex items-center gap-2">
              <div className="flex-1 rounded-lg border border-zinc-800 bg-zinc-900/60 px-3 py-2 font-mono text-sm text-zinc-300">
                0.0.0.0:9180
              </div>
              <Tag color={healthy ? "green" : "red"}>{healthy ? "Connected" : "Unreachable"}</Tag>
            </div>
            <p className="mt-1.5 text-[11px] text-zinc-600">
              Configured in <code className="rounded bg-zinc-800 px-1">ando.yaml</code> — requires server restart to change.
            </p>
          </div>
          <div>
            <label className="mb-1.5 block text-[11px] font-semibold uppercase tracking-widest text-zinc-500">
              Data Plane Listen Address
            </label>
            <div className="flex items-center gap-2">
              <div className="flex-1 rounded-lg border border-zinc-800 bg-zinc-900/60 px-3 py-2 font-mono text-sm text-zinc-300">
                0.0.0.0:9080
              </div>
              <Tag color="green">Listening</Tag>
            </div>
          </div>
          <div>
            <label className="mb-1.5 block text-[11px] font-semibold uppercase tracking-widest text-zinc-500">
              SSL Data Plane
            </label>
            <div className="flex items-center gap-2">
              <div className="flex-1 rounded-lg border border-zinc-800 bg-zinc-900/60 px-3 py-2 font-mono text-sm text-zinc-300">
                0.0.0.0:9443
              </div>
              <Tag color="zinc">TLS</Tag>
            </div>
          </div>
        </div>
      </Card>

      {/* Persistence */}
      <Card title="Persistence">
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <div className="text-[11px] font-semibold uppercase tracking-widest text-zinc-500 mb-0.5">Mode</div>
              <div className="font-mono text-sm text-zinc-200">
                {hasPersistence ? "File-based (JSON)" : "None — in-memory only"}
              </div>
            </div>
            <Tag color={hasPersistence ? (isFileHealthy ? "green" : "amber") : "red"}>
              {hasPersistence ? (isFileHealthy ? "Active" : "File missing") : "Disabled"}
            </Tag>
          </div>

          {hasPersistence && persistence.path && (
            <>
              <div>
                <div className="mb-1 text-[11px] font-semibold uppercase tracking-widest text-zinc-500">State File</div>
                <div className="rounded-lg border border-zinc-800 bg-zinc-900/60 px-3 py-2 font-mono text-xs text-zinc-300 break-all">
                  {persistence.path}
                </div>
              </div>
              <div className="grid grid-cols-3 gap-4">
                <div>
                  <div className="mb-0.5 text-[10px] font-semibold uppercase tracking-widest text-zinc-600">Status</div>
                  <div className={`text-xs font-semibold ${persistence.file_exists ? "text-green-400" : "text-amber-400"}`}>
                    {persistence.file_exists ? "File exists" : "Not yet created"}
                  </div>
                </div>
                <div>
                  <div className="mb-0.5 text-[10px] font-semibold uppercase tracking-widest text-zinc-600">Size</div>
                  <div className="font-mono text-xs text-zinc-300">
                    {persistence.size_bytes != null ? formatBytes(persistence.size_bytes) : "—"}
                  </div>
                </div>
                <div>
                  <div className="mb-0.5 text-[10px] font-semibold uppercase tracking-widest text-zinc-600">Last Write</div>
                  <div
                    className="font-mono text-xs text-zinc-300"
                    title={persistence.last_modified_unix != null ? formatUnixTime(persistence.last_modified_unix) : ""}
                  >
                    {persistence.last_modified_unix != null ? secondsAgo(persistence.last_modified_unix) : "—"}
                  </div>
                </div>
              </div>
              <p className="text-[11px] text-zinc-600">
                All routes, upstreams, services and consumers are saved atomically on every write and reloaded on restart.
              </p>
            </>
          )}

          {!hasPersistence && (
            <p className="text-[11px] text-amber-500/80">
              No state file is configured. All configuration will be lost on restart. Pass{" "}
              <code className="rounded bg-zinc-800 px-1 text-zinc-300">--state-file ./data/ando-state.json</code>{" "}
              to enable persistence.
            </p>
          )}
        </div>
      </Card>

      {/* Edition */}
      <Card title="Edition">
        <div className="flex items-start gap-4">
          <div className={`flex h-12 w-12 items-center justify-center rounded-xl ${isEnterprise ? "bg-indigo-900/30" : "bg-zinc-800/60"}`}>
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" className={isEnterprise ? "text-indigo-400" : "text-zinc-400"}>
              {isEnterprise
                ? <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                : <><polygon points="12 2 2 7 12 12 22 7 12 2" /><polyline points="2 17 12 22 22 17" /><polyline points="2 12 12 17 22 12" /></>}
            </svg>
          </div>
          <div className="flex-1">
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold text-zinc-200">
                Ando {isEnterprise ? "Enterprise" : "Community"} Edition
              </span>
              <Tag color={isEnterprise ? "indigo" : "green"}>Active</Tag>
            </div>
            <p className="mt-0.5 text-xs text-zinc-500">
              {isEnterprise
                ? "Enterprise license — all plugins and clustering features unlocked"
                : "Open-source API gateway — Apache 2.0 License"}
            </p>
            <div className="mt-3 flex items-center gap-3 text-xs text-zinc-600">
              {isEnterprise ? (
                <>
                  <span>All plugins unlocked</span>
                  <span className="text-zinc-800">&middot;</span>
                  <span>Cluster mode available</span>
                  <span className="text-zinc-800">&middot;</span>
                  <span>RBAC enabled</span>
                </>
              ) : (
                <>
                  <span>6 plugins enabled</span>
                  <span className="text-zinc-800">&middot;</span>
                  <span>Unlimited routes</span>
                  <span className="text-zinc-800">&middot;</span>
                  <span>Single-node deployment</span>
                </>
              )}
            </div>
          </div>
        </div>
      </Card>

      {/* Upgrade — CE only */}
      {!isEnterprise && (
        <Card title="Enterprise Upgrade">
          <div className="flex items-start justify-between gap-6">
            <div className="space-y-3 text-xs text-zinc-400">
              <p>Upgrade to Ando Enterprise Edition for advanced plugins, clustering, RBAC, audit logging, and priority support.</p>
              <ul className="space-y-1.5">
                {[
                  "12 enterprise plugins (OAuth2, HMAC, traffic mirror…)",
                  "Cluster mode with leader election",
                  "Role-based access control for Admin API",
                  "PII-aware audit logs",
                  "Priority email & Slack support",
                ].map((item) => (
                  <li key={item} className="flex items-center gap-2">
                    <svg className="h-3.5 w-3.5 shrink-0 text-green-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                    {item}
                  </li>
                ))}
              </ul>
            </div>
            <div className="shrink-0">
              <a href="https://andolabs.org/enterprise" target="_blank" rel="noopener">
                <Button variant="primary" size="md">Get Enterprise</Button>
              </a>
            </div>
          </div>
        </Card>
      )}

      {/* About */}
      <Card title="About">
        <div className="grid grid-cols-2 gap-4 text-xs sm:grid-cols-4">
          {[
            { label: "Version", value: version || "—" },
            { label: "Runtime", value: "monoio (io_uring)" },
            { label: "Language", value: "Rust 2024 edition" },
            { label: "Protocol", value: "HTTP/1.1, H2" },
          ].map((item) => (
            <div key={item.label}>
              <div className="mb-0.5 text-[10px] font-semibold uppercase tracking-widest text-zinc-600">{item.label}</div>
              <div className="font-mono text-zinc-300">{item.value}</div>
            </div>
          ))}
        </div>
      </Card>
    </div>
  );
}
