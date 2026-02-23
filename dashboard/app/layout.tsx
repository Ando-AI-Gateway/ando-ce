import type { Metadata } from "next";
import "./globals.css";
import { DashboardShell } from "@/components/shell";

export const metadata: Metadata = {
  title: "Ando — Dashboard",
  description: "Admin dashboard for Ando API Gateway",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <DashboardShell>{children}</DashboardShell>
      </body>
    </html>
  );
}
