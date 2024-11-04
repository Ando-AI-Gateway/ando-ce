import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Ando Dashboard — Enterprise API Gateway",
  description: "Next-generation API Gateway management console.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>
        <nav className="glass fixed top-0 w-full z-50 h-[var(--nav-height)] border-b border-[var(--surface-border)]">
          <div className="container h-full flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-[#00d2ff] to-[#9444fb] flex items-center justify-center font-bold text-white shadow-lg shadow-[#00d2ff]/20">
                A
              </div>
              <span className="text-xl font-bold tracking-tight">
                ANDO <span className="text-muted font-normal text-sm ml-1 opacity-50">DASHBOARD</span>
              </span>
            </div>

            <div className="flex items-center gap-8 text-sm font-medium">
              <a href="/" className="text-primary border-b-2 border-primary py-2">Overview</a>
              <a href="/routes" className="text-text-muted hover:text-white transition-colors">Routes</a>
              <a href="/upstreams" className="text-text-muted hover:text-white transition-colors">Upstreams</a>
              <a href="/consumers" className="text-text-muted hover:text-white transition-colors">Consumers</a>
              <a href="/observability" className="text-text-muted hover:text-white transition-colors">Observability</a>
            </div>

            <div className="flex items-center gap-4">
              <div className="flex items-center gap-2 text-xs text-text-muted bg-white/5 px-3 py-1.5 rounded-full border border-white/10">
                <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse"></span>
                ANDO CLUSTER RUNNING
              </div>
            </div>
          </div>
        </nav>

        <main className="pt-[calc(var(--nav-height)+40px)] container min-h-screen">
          {children}
        </main>
      </body>
    </html>
  );
}
