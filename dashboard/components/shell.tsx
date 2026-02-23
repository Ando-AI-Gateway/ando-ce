"use client";

import { DashboardProvider } from "@/lib/api";
import { Sidebar } from "@/components/sidebar";
import { Topbar } from "@/components/topbar";

export function DashboardShell({ children }: { children: React.ReactNode }) {
  return (
    <DashboardProvider>
      <div className="flex h-screen overflow-hidden">
        <Sidebar />
        <div className="flex flex-1 flex-col overflow-hidden">
          <Topbar />
          <main className="flex-1 overflow-y-auto p-5">{children}</main>
        </div>
      </div>
    </DashboardProvider>
  );
}
