export default function RoutesPage() {
    const routes = [
        { id: "1", name: "User Service API", uri: "/api/users/*", upstream: "user-upstream", status: "Active", methods: ["GET", "POST"] },
        { id: "2", name: "Payments Gateway", uri: "/v1/payments", upstream: "pay-cluster", status: "Active", methods: ["POST"] },
        { id: "3", name: "Legacy Auth", uri: "/auth/login", upstream: "legacy-auth", status: "Disabled", methods: ["ANY"] },
    ];

    return (
        <div className="animate-fade-in">
            <div className="flex items-center justify-between mb-12">
                <div>
                    <h1 className="text-4xl font-bold mb-2">Routes</h1>
                    <p className="text-text-muted">Manage URI path matching and plugin pipelines.</p>
                </div>
                <button className="px-6 py-3 rounded-xl bg-primary text-black font-bold hover:scale-105 transition-transform shadow-lg shadow-primary/20">
                    + Create New Route
                </button>
            </div>

            <div className="card overflow-hidden p-0 border-white/5">
                <table className="w-full text-left border-collapse">
                    <thead>
                        <tr className="bg-white/5 text-[10px] uppercase tracking-[0.2em] font-bold text-text-muted">
                            <th className="px-8 py-4">Name & URI</th>
                            <th className="px-8 py-4">Upstream</th>
                            <th className="px-8 py-4">Methods</th>
                            <th className="px-8 py-4">Status</th>
                            <th className="px-8 py-4 text-right">Actions</th>
                        </tr>
                    </thead>
                    <tbody className="divide-y divide-white/5">
                        {routes.map((route) => (
                            <tr key={route.id} className="hover:bg-white/[0.02] transition-colors group">
                                <td className="px-8 py-6">
                                    <div className="font-semibold mb-1 group-hover:text-primary transition-colors">{route.name}</div>
                                    <code className="text-xs text-text-muted bg-white/5 px-1.5 py-0.5 rounded">{route.uri}</code>
                                </td>
                                <td className="px-8 py-6">
                                    <div className="flex items-center gap-2">
                                        <div className="w-2 h-2 rounded-full bg-primary/40"></div>
                                        <span className="text-sm font-medium">{route.upstream}</span>
                                    </div>
                                </td>
                                <td className="px-8 py-6">
                                    <div className="flex gap-1">
                                        {route.methods.map(m => (
                                            <span key={m} className="px-2 py-0.5 rounded bg-white/5 text-[9px] font-bold tracking-wider">{m}</span>
                                        ))}
                                    </div>
                                </td>
                                <td className="px-8 py-6">
                                    <span className={`badge ${route.status === 'Active' ? 'badge-success' : 'bg-white/5 text-text-muted border border-white/10'}`}>
                                        {route.status}
                                    </span>
                                </td>
                                <td className="px-8 py-6 text-right">
                                    <button className="p-2 hover:bg-white/10 rounded-lg transition-colors text-text-muted hover:text-white">
                                        Edit
                                    </button>
                                    <button className="p-2 hover:bg-red-500/10 rounded-lg transition-colors text-text-muted hover:text-red-400">
                                        Delete
                                    </button>
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>
        </div>
    );
}
