export default function UpstreamsPage() {
    const upstreams = [
        { id: "1", name: "User Service Cluster", nodes: 3, type: "Round Robin", health: "100%", latency: "2.4ms" },
        { id: "2", name: "Payment Gateway API", nodes: 2, type: "Least Connections", health: "50%", latency: "1.2ms" },
        { id: "3", name: "Legacy Auth DB", nodes: 1, type: "Round Robin", health: "Active", latency: "12ms" },
    ];

    return (
        <div className="animate-fade-in">
            <div className="flex items-center justify-between mb-12">
                <div>
                    <h1 className="text-4xl font-bold mb-2">Upstreams</h1>
                    <p className="text-text-muted">Target backend server pools and health check configuration.</p>
                </div>
                <button className="px-6 py-3 rounded-xl bg-primary text-black font-bold hover:scale-105 transition-transform shadow-lg shadow-primary/20">
                    + Add New Upstream
                </button>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
                {upstreams.map((u) => (
                    <div key={u.id} className="card group">
                        <div className="flex justify-between items-start mb-6">
                            <h3 className="text-xl font-bold group-hover:text-primary transition-colors">{u.name}</h3>
                            <span className={`badge ${u.health === '100%' ? 'badge-success' : 'badge-primary'}`}>{u.health}</span>
                        </div>

                        <div className="space-y-4 mb-8">
                            <div className="flex justify-between text-sm">
                                <span className="text-text-muted">Algorithm</span>
                                <span className="font-medium">{u.type}</span>
                            </div>
                            <div className="flex justify-between text-sm">
                                <span className="text-text-muted">Backend Nodes</span>
                                <span className="font-medium text-primary">{u.nodes} Servers</span>
                            </div>
                            <div className="flex justify-between text-sm">
                                <span className="text-text-muted">Avg Latency</span>
                                <span className="font-medium text-[#22c55e]">{u.latency}</span>
                            </div>
                        </div>

                        <div className="flex gap-2">
                            <button className="flex-1 py-2 rounded-lg bg-white/5 border border-white/10 text-xs font-bold uppercase tracking-wider hover:bg-white/10 transition-colors">Configure</button>
                            <button className="px-4 py-2 rounded-lg bg-white/5 border border-white/10 text-xs font-bold text-red-400 hover:bg-red-500/10 transition-colors">Delete</button>
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}
