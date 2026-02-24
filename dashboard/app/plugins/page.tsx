"use client";

import { useState, type ReactNode } from "react";
import { CE_PLUGINS, EE_PLUGINS, type PluginInfo } from "@/lib/plugins";
import { useDashboard } from "@/lib/api";
import { Tag, Modal, Button } from "@/components/ui";

const ICONS: Record<string, (size?: number) => ReactNode> = {
  key: (s = 18) => (
    <svg width={s} height={s} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7">
      <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4" />
    </svg>
  ),
  shield: (s = 18) => (
    <svg width={s} height={s} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7">
      <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
    </svg>
  ),
  user: (s = 18) => (
    <svg width={s} height={s} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7">
      <path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2" /><circle cx="12" cy="7" r="4" />
    </svg>
  ),
  globe: (s = 18) => (
    <svg width={s} height={s} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7">
      <circle cx="12" cy="12" r="10" /><line x1="2" y1="12" x2="22" y2="12" />
      <path d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z" />
    </svg>
  ),
  activity: (s = 18) => (
    <svg width={s} height={s} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7">
      <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
    </svg>
  ),
  layers: (s = 18) => (
    <svg width={s} height={s} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7">
      <polygon points="12 2 2 7 12 12 22 7 12 2" />
      <polyline points="2 17 12 22 22 17" /><polyline points="2 12 12 17 22 12" />
    </svg>
  ),
  lock: (s = 18) => (
    <svg width={s} height={s} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7">
      <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
      <path d="M7 11V7a5 5 0 0110 0v4" />
    </svg>
  ),
};

function getIcon(name: string, size?: number) {
  return (ICONS[name] ?? ICONS.lock)(size);
}

function PluginCard({ plugin, locked, onClick }: { plugin: PluginInfo; locked?: boolean; onClick?: () => void }) {
  return (
    <div
      onClick={onClick}
      className={`group relative rounded-xl border p-4 transition-all ${
        locked
          ? "cursor-pointer border-stone-800 bg-stone-950/60 opacity-75 hover:border-stone-700 hover:opacity-90"
          : "border-zinc-800 bg-zinc-950/60 hover:border-zinc-700"
      }`}
    >
      <div className="mb-3 flex items-start justify-between">
        <div className={`flex h-9 w-9 items-center justify-center rounded-lg ${locked ? "bg-stone-800/40 text-stone-600" : "bg-white/[0.06] text-zinc-400"}`}>
          {getIcon(plugin.icon)}
        </div>
        <div className="flex gap-1.5">
          {locked ? <Tag color="ee">Enterprise</Tag> : <Tag color="green">Active</Tag>}
          <Tag color="zinc">{plugin.phase}</Tag>
        </div>
      </div>
      <div className={`mb-1 font-mono text-[13px] font-semibold ${locked ? "text-stone-600" : "text-zinc-100"}`}>
        {plugin.name}
      </div>
      <p className={`text-xs leading-relaxed ${locked ? "text-stone-700" : "text-zinc-400"}`}>
        {plugin.desc}
      </p>
      {locked && (
        <div className="absolute inset-0 flex items-center justify-center rounded-xl bg-stone-950/85 opacity-0 backdrop-blur-sm transition-opacity group-hover:opacity-100">
          <div className="text-center">
            <div className="mx-auto mb-2 text-stone-500">{getIcon("lock", 22)}</div>
            <div className="text-[13px] font-semibold text-stone-300">Enterprise Feature</div>
            <div className="mb-3 text-[11px] text-stone-500">Requires Ando Enterprise Edition</div>
            <span className="inline-block rounded-md bg-stone-300 px-4 py-1.5 text-[11px] font-semibold text-stone-900 transition-colors hover:bg-white">
              Upgrade to Unlock
            </span>
          </div>
        </div>
      )}
    </div>
  );
}

export default function PluginsPage() {
  const [selectedPlugin, setSelectedPlugin] = useState<PluginInfo | null>(null);
  const { edition } = useDashboard();
  const isEnterprise = edition === "enterprise";

  return (
    <div className="space-y-6">
      {/* Notice */}
      <div className={`flex items-start gap-3 rounded-xl border p-4 ${
        isEnterprise
          ? "border-indigo-500/30 bg-indigo-500/5"
          : "border-amber-500/20 bg-amber-500/5"
      }`}>
        <svg className={`mt-0.5 h-4 w-4 shrink-0 ${isEnterprise ? "text-indigo-400" : "text-amber-500"}`} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          {isEnterprise
            ? <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
            : <><circle cx="12" cy="12" r="10" /><path d="M12 8v4M12 16h.01" /></>}
        </svg>
        <div className="text-xs leading-relaxed text-zinc-400">
          {isEnterprise ? (
            <>
              You are running{" "}
              <span className="font-semibold text-zinc-200">Ando Enterprise Edition</span>.
              All {CE_PLUGINS.length + EE_PLUGINS.length} plugins are unlocked and available.
            </>
          ) : (
            <>
              You are running{" "}
              <span className="font-semibold text-zinc-200">Ando Community Edition</span>.
              Enterprise plugins are visible below but require a license to enable.{" "}
              <a href="https://andolabs.org/enterprise" target="_blank" rel="noopener" className="font-semibold text-amber-500 hover:text-amber-400">
                Learn more &rarr;
              </a>
            </>
          )}
        </div>
      </div>

      {/* CE Plugins */}
      <div>
        <div className="mb-1 text-xs font-semibold text-zinc-300">
          Community Edition &middot; {CE_PLUGINS.length} plugins available
        </div>
        <p className="mb-4 text-xs text-zinc-600">All CE plugins are active and ready to attach to routes and consumers.</p>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {CE_PLUGINS.map((p) => <PluginCard key={p.name} plugin={p} />)}
        </div>
      </div>

      {/* Divider */}
      <div className="flex items-center gap-3">
        <div className="h-px flex-1 bg-zinc-800" />
        <div className="flex items-center gap-1.5 whitespace-nowrap text-[10px] font-semibold uppercase tracking-widest text-zinc-600">
          {isEnterprise ? (
            <><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /></svg> Enterprise Edition &middot; {EE_PLUGINS.length} plugins active</>
          ) : (
            <>{getIcon("lock", 12)} Enterprise Edition &middot; {EE_PLUGINS.length} plugins locked</>
          )}
        </div>
        <div className="h-px flex-1 bg-zinc-800" />
      </div>

      {/* EE Plugins */}
      <div>
        <p className="mb-4 text-xs text-zinc-600">
          {isEnterprise
            ? "Enterprise plugins are active on this instance."
            : "These plugins require Ando Enterprise Edition. Hover to see details."}
        </p>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {EE_PLUGINS.map((p) => (
            <PluginCard
              key={p.name}
              plugin={p}
              locked={!isEnterprise}
              onClick={!isEnterprise ? () => setSelectedPlugin(p) : undefined}
            />
          ))}
        </div>
      </div>

      {/* EE Detail modal */}
      <Modal open={!!selectedPlugin} onClose={() => setSelectedPlugin(null)} title={selectedPlugin?.name ?? ""}>
        {selectedPlugin && (
          <>
            <div className="mb-4 inline-flex items-center gap-1.5 rounded-full border border-stone-700 bg-stone-800/50 px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-stone-400">
              {getIcon("lock", 12)} Enterprise Only
            </div>
            <p className="mb-5 text-sm leading-relaxed text-zinc-400">{selectedPlugin.desc}</p>
            {selectedPlugin.features && selectedPlugin.features.length > 0 && (
              <div className="mb-6 space-y-2">
                {selectedPlugin.features.map((f) => (
                  <div key={f} className="flex items-center gap-2 text-xs text-zinc-400">
                    <svg className="h-3.5 w-3.5 shrink-0 text-green-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                    {f}
                  </div>
                ))}
              </div>
            )}
            <div className="flex gap-2">
              <a href="https://andolabs.org/enterprise" target="_blank" rel="noopener"
                className="inline-flex items-center justify-center rounded-lg bg-white px-4 py-2 text-xs font-semibold text-zinc-900 transition-colors hover:bg-zinc-200">
                Get Enterprise Edition
              </a>
              <Button variant="secondary" size="md" onClick={() => setSelectedPlugin(null)}>Dismiss</Button>
            </div>
          </>
        )}
      </Modal>
    </div>
  );
}
