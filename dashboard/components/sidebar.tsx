"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useDashboard } from "@/lib/api";

type SectionHeader = { label: string };
type NavItem = {
  href: string;
  label: string;
  icon: React.ReactNode;
  countKey?: "routes" | "upstreams" | "consumers";
};
type SidebarItem = SectionHeader | NavItem;

function isNavItem(item: SidebarItem): item is NavItem {
  return "href" in item;
}

const sections: SidebarItem[] = [
  { label: "GATEWAY" },
  {
    href: "/",
    label: "Overview",
    icon: (
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <rect x="3" y="3" width="7" height="7" rx="1" />
        <rect x="14" y="3" width="7" height="7" rx="1" />
        <rect x="3" y="14" width="7" height="7" rx="1" />
        <rect x="14" y="14" width="7" height="7" rx="1" />
      </svg>
    ),
  },
  {
    href: "/routes",
    label: "Routes",
    countKey: "routes" as const,
    icon: (
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <path d="M18 8h1a4 4 0 010 8h-1M2 8h16v9a4 4 0 01-4 4H6a4 4 0 01-4-4V8z" />
        <line x1="6" y1="1" x2="6" y2="4" />
        <line x1="10" y1="1" x2="10" y2="4" />
        <line x1="14" y1="1" x2="14" y2="4" />
      </svg>
    ),
  },
  {
    href: "/upstreams",
    label: "Upstreams",
    countKey: "upstreams" as const,
    icon: (
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <circle cx="12" cy="12" r="10" />
        <line x1="2" y1="12" x2="22" y2="12" />
        <path d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z" />
      </svg>
    ),
  },
  {
    href: "/consumers",
    label: "Consumers",
    countKey: "consumers" as const,
    icon: (
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2" />
        <circle cx="9" cy="7" r="4" />
        <path d="M23 21v-2a4 4 0 00-3-3.87" />
        <path d="M16 3.13a4 4 0 010 7.75" />
      </svg>
    ),
  },
  {
    href: "/plugins",
    label: "Plugins",
    icon: (
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <path d="M12 2L2 7l10 5 10-5-10-5z" />
        <path d="M2 17l10 5 10-5" />
        <path d="M2 12l10 5 10-5" />
      </svg>
    ),
  },
  { label: "SYSTEM" },
  {
    href: "/settings",
    label: "Settings",
    icon: (
      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
      </svg>
    ),
  },
];

export function Sidebar() {
  const pathname = usePathname();
  const { routes, upstreams, consumers } = useDashboard();

  const counts: Record<string, number> = {
    routes: routes.length,
    upstreams: upstreams.length,
    consumers: consumers.length,
  };

  return (
    <aside className="flex w-[220px] min-w-[220px] flex-col border-r border-zinc-800 bg-zinc-950/80">
      {/* Logo */}
      <div className="flex items-center gap-2.5 border-b border-zinc-800 px-4 py-4">
        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-white">
          <svg className="h-4 w-4 text-black" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
            <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" />
          </svg>
        </div>
        <div>
          <div className="text-sm font-semibold tracking-tight">Ando</div>
          <div className="text-[10px] font-medium uppercase tracking-wider text-zinc-500">
            Community Edition
          </div>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto py-2">
        {sections.map((item, i) => {
          // Section header
          if (!isNavItem(item)) {
            return (
              <div
                key={i}
                className="px-4 pb-1 pt-4 text-[10px] font-semibold uppercase tracking-widest text-zinc-600"
              >
                {item.label}
              </div>
            );
          }

          // basePath is /dashboard, so internal hrefs are relative to that.
          // pathname from usePathname() includes basePath: /dashboard, /dashboard/routes, etc.
          const fullHref = item.href === "/" ? "/dashboard" : `/dashboard${item.href}`;
          const active =
            item.href === "/"
              ? pathname === "/dashboard" || pathname === "/dashboard/"
              : pathname.startsWith(fullHref);

          return (
            <Link
              key={item.href}
              href={item.href}
              className={`flex items-center gap-2.5 border-l-2 px-4 py-[7px] text-[13px] transition-colors ${
                active
                  ? "border-white bg-white/[0.06] text-zinc-50"
                  : "border-transparent text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200"
              }`}
            >
              {item.icon}
              <span className="flex-1">{item.label}</span>
              {item.countKey && counts[item.countKey] !== undefined && (
                <span className="rounded-full bg-white/[0.08] px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-zinc-500">
                  {counts[item.countKey]}
                </span>
              )}
            </Link>
          );
        })}
      </nav>

      {/* Upgrade card */}
      <div className="mx-3 mb-3 rounded-lg border border-stone-800 bg-stone-900/40 p-3">
        <div className="mb-1 text-[10px] font-bold uppercase tracking-wider text-stone-500">
          Enterprise Edition
        </div>
        <p className="mb-2 text-[11px] leading-relaxed text-stone-600">
          Unlock 6 more plugins, clustering, RBAC &amp; priority support.
        </p>
        <a
          href="https://andolabs.org/enterprise"
          target="_blank"
          rel="noopener"
          className="inline-block rounded-md bg-stone-700 px-2.5 py-1 text-[10px] font-semibold text-stone-200 transition-colors hover:bg-stone-600"
        >
          Learn More →
        </a>
      </div>
    </aside>
  );
}
