"use client";

import { Card, Button, Tag } from "@/components/ui";

export default function SettingsPage() {
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
              <Tag color="green">Connected</Tag>
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

      {/* Edition */}
      <Card title="Edition">
        <div className="flex items-start gap-4">
          <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-zinc-800/60">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" className="text-zinc-400">
              <polygon points="12 2 2 7 12 12 22 7 12 2" />
              <polyline points="2 17 12 22 22 17" />
              <polyline points="2 12 12 17 22 12" />
            </svg>
          </div>
          <div className="flex-1">
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold text-zinc-200">Ando Community Edition</span>
              <Tag color="green">Active</Tag>
            </div>
            <p className="mt-0.5 text-xs text-zinc-500">Open-source API gateway — Apache 2.0 License</p>
            <div className="mt-3 flex items-center gap-3 text-xs text-zinc-600">
              <span>6 plugins enabled</span>
              <span className="text-zinc-800">&middot;</span>
              <span>Unlimited routes</span>
              <span className="text-zinc-800">&middot;</span>
              <span>Single-node deployment</span>
            </div>
          </div>
        </div>
      </Card>

      {/* Upgrade */}
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

      {/* About */}
      <Card title="About">
        <div className="grid grid-cols-2 gap-4 text-xs sm:grid-cols-4">
          {[
            { label: "Runtime", value: "monoio (io_uring)" },
            { label: "Language", value: "Rust 2024 edition" },
            { label: "Protocol", value: "HTTP/1.1, H2" },
            { label: "Config", value: "YAML + Admin API" },
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
