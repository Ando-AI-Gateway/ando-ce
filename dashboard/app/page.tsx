"use client";

import { useDashboard } from "@/lib/api";
import { StatCard, Card, Tag, EmptyState } from "@/components/ui";
import { CE_PLUGINS, EE_PLUGINS, COMPARISON_ROWS } from "@/lib/plugins";

export default function OverviewPage() {
  const { routes, upstreams, consumers, loading } = useDashboard();

  if (loading) {
    return (
      <div className="flex h-48 items-center justify-center text-sm text-zinc-500">
        Loading…
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Stats */}
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatCard label="Routes" value={routes.length} sub="Active routing rules" />
        <StatCard label="Upstreams" value={upstreams.length} sub="Backend targets" />
        <StatCard label="Consumers" value={consumers.length} sub="API consumers" />
        <StatCard label="Plugins" value={CE_PLUGINS.length} sub={`+${EE_PLUGINS.length} in Enterprise`} />
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {/* Recent routes */}
        <Card title="Recent Routes">
          {routes.length === 0 ? (
            <EmptyState message="No routes configured" />
          ) : (
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b border-zinc-800 text-left text-[10px] font-semibold uppercase tracking-widest text-zinc-500">
                  <th className="pb-2 pr-4">ID</th>
                  <th className="pb-2 pr-4">URI</th>
                  <th className="pb-2 pr-4">Methods</th>
                  <th className="pb-2">Status</th>
                </tr>
              </thead>
              <tbody>
                {routes.slice(0, 5).map((r) => (
                  <tr key={r.id} className="border-b border-zinc-800/50">
                    <td className="py-2 pr-4 font-mono text-zinc-300">{r.id}</td>
                    <td className="py-2 pr-4 text-zinc-400">{r.uri}</td>
                    <td className="py-2 pr-4">
                      {(r.methods ?? ["*"]).map((m) => (
                        <Tag key={m} color="blue">{m}</Tag>
                      ))}
                    </td>
                    <td className="py-2">
                      <Tag color={r.status === 0 ? "red" : "green"}>
                        {r.status === 0 ? "Off" : "On"}
                      </Tag>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Card>

        {/* Plugin summary */}
        <Card title="Plugins">
          <div className="space-y-2">
            {CE_PLUGINS.map((p) => (
              <div key={p.name} className="flex items-center justify-between text-xs">
                <div className="flex items-center gap-2">
                  <span className="font-mono text-zinc-300">{p.name}</span>
                  <Tag color="zinc">{p.phase}</Tag>
                </div>
                <Tag color="green">Active</Tag>
              </div>
            ))}
            {EE_PLUGINS.slice(0, 3).map((p) => (
              <div key={p.name} className="flex items-center justify-between text-xs">
                <div className="flex items-center gap-2">
                  <span className="font-mono text-stone-600">{p.name}</span>
                  <Tag color="zinc">{p.phase}</Tag>
                </div>
                <Tag color="ee">Locked</Tag>
              </div>
            ))}
          </div>
        </Card>
      </div>

      {/* Edition comparison */}
      <Card title="Edition Comparison">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-zinc-800 text-left text-[10px] font-semibold uppercase tracking-widest text-zinc-500">
              <th className="pb-2 pr-4">Feature</th>
              <th className="pb-2 pr-4 text-center">CE</th>
              <th className="pb-2 text-center">Enterprise</th>
            </tr>
          </thead>
          <tbody>
            {COMPARISON_ROWS.map((row) => (
              <tr key={row.feature} className="border-b border-zinc-800/40">
                <td className="py-1.5 pr-4 text-zinc-400">{row.feature}</td>
                <td className="py-1.5 text-center">
                  {row.ce ? (
                    <span className="text-green-500">✓</span>
                  ) : (
                    <span className="text-zinc-700">—</span>
                  )}
                </td>
                <td className="py-1.5 text-center">
                  {row.ee ? (
                    <span className="text-green-500">✓</span>
                  ) : (
                    <span className="text-zinc-700">—</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>
    </div>
  );
}
