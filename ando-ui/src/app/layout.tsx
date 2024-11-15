import type { Metadata } from "next";
import "./globals.css";
import Sidebar from "@/components/Sidebar";

export const metadata: Metadata = {
  title: "Ando Dashboard — Enterprise API Gateway",
  description: "Manage routes, upstreams, plugins, and observe your API gateway in real-time.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <div className="app-shell">
          <Sidebar />
          <div className="main-content">
            <div className="page-content fade-in">
              {children}
            </div>
          </div>
        </div>
      </body>
    </html>
  );
}
