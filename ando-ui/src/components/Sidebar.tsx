'use client';

import { usePathname } from 'next/navigation';
import Link from 'next/link';
import { useEffect, useState } from 'react';
import { health, HealthResponse } from '@/lib/api';

interface NavItem {
    href: string;
    icon: string;
    label: string;
    exact?: boolean;
}

const NAV_SECTIONS: { title: string; items: NavItem[] }[] = [
    {
        title: 'Dashboard',
        items: [
            { href: '/', icon: '◎', label: 'Overview', exact: true },
        ],
    },
    {
        title: 'Gateway',
        items: [
            { href: '/routes', icon: '⇢', label: 'Routes' },
            { href: '/services', icon: '◈', label: 'Services' },
            { href: '/upstreams', icon: '⬡', label: 'Upstreams' },
        ],
    },
    {
        title: 'Security',
        items: [
            { href: '/consumers', icon: '⊕', label: 'Consumers' },
            { href: '/ssl', icon: '⊘', label: 'SSL Certificates' },
        ],
    },
    {
        title: 'Extensions',
        items: [
            { href: '/plugins', icon: '⧉', label: 'Plugins' },
        ],
    },
    {
        title: 'Observability',
        items: [
            { href: '/observability', icon: '◑', label: 'Metrics & Logs' },
        ],
    },
];

export default function Sidebar() {
    const pathname = usePathname();
    const [status, setStatus] = useState<HealthResponse | null>(null);
    const [offline, setOffline] = useState(false);

    useEffect(() => {
        const fetch = () => {
            health.check()
                .then((h) => { setStatus(h); setOffline(false); })
                .catch(() => { setStatus(null); setOffline(true); });
        };
        fetch();
        const id = setInterval(fetch, 10000);
        return () => clearInterval(id);
    }, []);

    const isActive = (href: string, exact?: boolean) => {
        if (exact) return pathname === href;
        return pathname === href || pathname.startsWith(href + '/');
    };

    return (
        <aside className="sidebar">
            <div className="sidebar-header">
                <div className="sidebar-logo">
                    <div className="sidebar-logo-icon">A</div>
                    <div>
                        <div className="sidebar-logo-text">ANDO</div>
                        <div className="sidebar-logo-sub">API Gateway</div>
                    </div>
                </div>
            </div>

            <nav className="sidebar-nav">
                {NAV_SECTIONS.map((section) => (
                    <div key={section.title} className="sidebar-section">
                        <div className="sidebar-section-title">{section.title}</div>
                        {section.items.map((item) => (
                            <Link
                                key={item.href}
                                href={item.href}
                                className={`nav-item ${isActive(item.href, item.exact) ? 'active' : ''}`}
                            >
                                <span className="nav-icon">{item.icon}</span>
                                {item.label}
                            </Link>
                        ))}
                    </div>
                ))}
            </nav>

            <div className="sidebar-footer">
                {status ? (
                    <div className="sidebar-status">
                        <span className="status-dot" />
                        v{status.version} — {status.cache.routes}r / {status.cache.upstreams}u / {status.plugins_loaded}p
                    </div>
                ) : (
                    <div className="sidebar-status" style={{
                        background: offline ? 'var(--red-dim)' : 'var(--bg-hover)',
                        borderColor: offline ? 'rgba(255,77,106,0.2)' : 'var(--border)',
                        color: offline ? 'var(--red)' : 'var(--text-tertiary)',
                    }}>
                        <span className="status-dot" style={{
                            background: offline ? 'var(--red)' : 'var(--text-tertiary)',
                            boxShadow: offline ? '0 0 8px var(--red)' : 'none',
                        }} />
                        {offline ? 'Ando offline' : 'Connecting…'}
                    </div>
                )}
            </div>
        </aside>
    );
}
