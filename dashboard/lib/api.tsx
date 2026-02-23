"use client";

import { createContext, useContext, useState, useCallback, useEffect, type ReactNode } from "react";

// ── Types ────────────────────────────────────────────────────────
export interface Route {
  id: string;
  name?: string;
  uri: string;
  methods?: string[];
  upstream_id?: string;
  plugins?: Record<string, unknown>;
  status?: number;
}

export interface Upstream {
  id?: string;
  name?: string;
  nodes: Record<string, number>;
  type?: string;
}

export interface Consumer {
  username: string;
  plugins?: Record<string, unknown>;
  create_time?: number;
}

interface DashboardState {
  routes: Route[];
  upstreams: Upstream[];
  consumers: Consumer[];
  healthy: boolean;
  loading: boolean;
  error: string | null;
}

interface DashboardCtx extends DashboardState {
  refresh: () => Promise<void>;
  apiBase: string;
}

const DashboardContext = createContext<DashboardCtx | null>(null);

export function useDashboard() {
  const ctx = useContext(DashboardContext);
  if (!ctx) throw new Error("useDashboard must be used within DashboardProvider");
  return ctx;
}

// Admin API base — same origin by default (dashboard is served by admin API).
// Override with env var for dev mode.
const API_BASE =
  process.env.NEXT_PUBLIC_ADMIN_API_URL ?? "/apisix/admin";

// ── Provider ────────────────────────────────────────────────────
export function DashboardProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<DashboardState>({
    routes: [],
    upstreams: [],
    consumers: [],
    healthy: true,
    loading: true,
    error: null,
  });

  const refresh = useCallback(async () => {
    try {
      const [routesRes, upstreamsRes, consumersRes, healthRes] =
        await Promise.all([
          fetch(`${API_BASE}/routes`).then((r) => (r.ok ? r.json() : null)),
          fetch(`${API_BASE}/upstreams`).then((r) => (r.ok ? r.json() : null)),
          fetch(`${API_BASE}/consumers`).then((r) => (r.ok ? r.json() : null)),
          fetch(`${API_BASE}/health`).then((r) => (r.ok ? r.json() : null)),
        ]);

      setState({
        routes: routesRes?.list ?? routesRes?.routes ?? [],
        upstreams: upstreamsRes?.list ?? upstreamsRes?.upstreams ?? [],
        consumers: consumersRes?.list ?? consumersRes?.consumers ?? [],
        healthy: healthRes?.status === "ok" || !!healthRes,
        loading: false,
        error: null,
      });
    } catch (e) {
      setState((prev) => ({
        ...prev,
        loading: false,
        healthy: false,
        error: e instanceof Error ? e.message : "Network error",
      }));
    }
  }, []);

  // Initial load + polling
  useEffect(() => {
    refresh();
    const iv = setInterval(refresh, 10_000);
    return () => clearInterval(iv);
  }, [refresh]);

  return (
    <DashboardContext.Provider value={{ ...state, refresh, apiBase: API_BASE }}>
      {children}
    </DashboardContext.Provider>
  );
}

// ── Mutation helpers ─────────────────────────────────────────────
export async function apiPut(
  path: string,
  body: unknown,
): Promise<{ ok: boolean; data?: unknown; error?: string }> {
  try {
    const r = await fetch(`${API_BASE}${path}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    const data = await r.json().catch(() => null);
    return r.ok ? { ok: true, data } : { ok: false, error: data?.error ?? r.statusText };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : "Network error" };
  }
}

export async function apiDelete(
  path: string,
): Promise<{ ok: boolean; error?: string }> {
  try {
    const r = await fetch(`${API_BASE}${path}`, { method: "DELETE" });
    return r.ok ? { ok: true } : { ok: false, error: r.statusText };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : "Network error" };
  }
}
