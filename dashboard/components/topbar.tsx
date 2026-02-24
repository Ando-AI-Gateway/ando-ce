"use client";

import { usePathname } from "next/navigation";
import { useDashboard } from "@/lib/api";
import { Tag } from "@/components/ui";

const META: Record<string, { title: string; sub: string }> = {
  "/dashboard": { title: "Overview", sub: "Gateway status and quick metrics" },
  "/dashboard/routes": { title: "Routes", sub: "Manage routing rules" },
  "/dashboard/upstreams": { title: "Upstreams", sub: "Backend service targets" },
  "/dashboard/consumers": { title: "Consumers", sub: "API consumer credentials" },
  "/dashboard/plugins": { title: "Plugins", sub: "CE and Enterprise plugins" },
  "/dashboard/settings": { title: "Settings", sub: "Connection and edition info" },
};

export function Topbar() {
  const pathname = usePathname();
  const { healthy, edition } = useDashboard();
  const isEnterprise = edition === "enterprise";

  // Strip trailing slash for lookup
  const key = pathname.replace(/\/$/, "") || "/dashboard";
  const meta = META[key] ?? { title: "Dashboard", sub: "" };

  return (
    <header className="flex h-12 items-center justify-between border-b border-zinc-800 px-5">
      <div>
        <h1 className="text-sm font-semibold text-zinc-200">{meta.title}</h1>
        {meta.sub && (
          <p className="text-[11px] text-zinc-500">{meta.sub}</p>
        )}
      </div>
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1.5 text-[11px] text-zinc-500">
          <span
            className={`inline-block h-2 w-2 rounded-full ${
              healthy ? "bg-green-500" : "bg-red-500"
            }`}
          />
          {healthy ? "Healthy" : "Unhealthy"}
        </div>
        <Tag color={isEnterprise ? "indigo" : "zinc"}>{isEnterprise ? "EE" : "CE"}</Tag>
      </div>
    </header>
  );
}
