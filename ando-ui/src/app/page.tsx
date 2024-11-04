import styles from "./page.module.css";

export default function Home() {
  return (
    <div className="animate-fade-in">
      <header className="mb-12">
        <h1 className="text-4xl font-bold mb-2">Systems Overview</h1>
        <p className="text-text-muted">Real-time status and telemetry from your Ando API Gateway nodes.</p>
      </header>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-12">
        <StatCard title="Total Requests" value="1.2M" subValue="+12% from last hour" color="var(--primary)" />
        <StatCard title="Avg Latency" value="4.2ms" subValue="-0.5ms improvement" color="#22c55e" />
        <StatCard title="Active Routes" value="124" subValue="3 newly added today" color="var(--secondary)" />
        <StatCard title="Uptime" value="99.998%" subValue="Last incidental: 12 days ago" color="#ff00c8" />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
        {/* Main Chart Area placeholder */}
        <div className="lg:col-span-2 card">
          <div className="flex items-center justify-between mb-8">
            <h3 className="text-lg font-semibold">Traffic Distribution</h3>
            <div className="flex gap-2">
              <button className="px-3 py-1 text-xs rounded-md bg-white/10 hover:bg-white/20 transition-colors">1H</button>
              <button className="px-3 py-1 text-xs rounded-md bg-primary text-black font-semibold">24H</button>
              <button className="px-3 py-1 text-xs rounded-md bg-white/10 hover:bg-white/20 transition-colors">7D</button>
            </div>
          </div>
          <div className="h-[300px] w-full flex items-end gap-2 px-2">
            {[40, 60, 45, 90, 65, 80, 55, 70, 85, 40, 50, 95, 30, 60, 80, 75, 50, 90, 40, 65, 85, 70, 45, 100].map((h, i) => (
              <div
                key={i}
                className="flex-1 bg-gradient-to-t from-primary/20 to-primary/60 rounded-t-sm transition-all hover:to-primary"
                style={{ height: `${h}%` }}
              ></div>
            ))}
          </div>
          <div className="flex justify-between mt-4 text-[10px] text-text-muted uppercase tracking-widest px-1">
            <span>00:00</span>
            <span>06:00</span>
            <span>12:00</span>
            <span>18:00</span>
            <span>23:59</span>
          </div>
        </div>

        {/* Info Column */}
        <div className="flex flex-col gap-6">
          <div className="card">
            <h3 className="text-lg font-semibold mb-6">Recent Activity</h3>
            <div className="space-y-6">
              <ActivityItem
                time="2m ago"
                title="Route Created"
                desc="/v1/payments -> payments-svc"
                icon="+"
                iconColor="var(--primary)"
              />
              <ActivityItem
                time="15m ago"
                title="Upstream Healthy"
                desc="auth-server-02 is back online"
                icon="✓"
                iconColor="#22c55e"
              />
              <ActivityItem
                time="45m ago"
                title="Limit Triggered"
                desc="IP 192.168.1.1 burst exceeded"
                icon="!!"
                iconColor="#ef4444"
              />
            </div>
          </div>

          <div className="card bg-gradient-to-br from-[#1a1a1f] to-[#0a0a0c]">
            <h3 className="text-lg font-semibold mb-2">Version Control</h3>
            <p className="text-xs text-text-muted mb-4">You are running Ando Enterprise v0.1.0-beta.1</p>
            <button className="w-full py-2.5 rounded-lg bg-white/5 border border-white/10 text-sm font-medium hover:bg-white/10 transition-colors">
              Check for Updates
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function StatCard({ title, value, subValue, color }: { title: string, value: string, subValue: string, color: string }) {
  return (
    <div className="card">
      <p className="text-xs font-semibold text-text-muted uppercase tracking-wider mb-2">{title}</p>
      <div className="flex items-baseline gap-2 mb-1">
        <h2 className="text-3xl font-bold">{value}</h2>
      </div>
      <p className="text-[10px] text-text-muted">
        <span style={{ color }}>{subValue.split(' ')[0]}</span> {subValue.split(' ').slice(1).join(' ')}
      </p>
    </div>
  );
}

function ActivityItem({ time, title, desc, icon, iconColor }: { time: string, title: string, desc: string, icon: string, iconColor: string }) {
  return (
    <div className="flex gap-4">
      <div
        className="w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold flex-shrink-0"
        style={{ border: `1px solid ${iconColor}44`, backgroundColor: `${iconColor}11`, color: iconColor }}
      >
        {icon}
      </div>
      <div>
        <p className="text-sm font-semibold">{title}</p>
        <p className="text-xs text-text-muted mb-1">{desc}</p>
        <p className="text-[10px] uppercase text-text-muted/60 tracking-wider font-bold">{time}</p>
      </div>
    </div>
  );
}
