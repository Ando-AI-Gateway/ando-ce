// Ando Admin API Client
// All CRUD operations for Routes, Services, Upstreams, Consumers, SSL, Plugins

const BASE = typeof window !== 'undefined'
    ? (window.location.port === '3000' ? 'http://localhost:9180' : '')
    : '';

const API = `${BASE}/apisix/admin`;

async function request<T>(path: string, opts?: RequestInit): Promise<T> {
    const res = await fetch(`${API}${path}`, {
        headers: { 'Content-Type': 'application/json', ...opts?.headers },
        ...opts,
    });
    if (!res.ok) {
        const text = await res.text().catch(() => '');
        throw new Error(`API ${res.status}: ${text || res.statusText}`);
    }
    if (res.status === 204) return undefined as T;
    return res.json();
}

// ─── Types ────────────────────────────────────────────────────────

export interface Route {
    id: string;
    name: string;
    description: string;
    uri: string;
    uris: string[];
    methods: string[];
    host?: string;
    hosts: string[];
    remote_addrs: string[];
    priority: number;
    enable: boolean;
    upstream?: InlineUpstream;
    upstream_id?: string;
    service_id?: string;
    plugins: Record<string, any>;
    plugin_config_id?: string;
    labels: Record<string, string>;
    status: number;
    timeout?: TimeoutConfig;
    created_at?: string;
    updated_at?: string;
}

export interface InlineUpstream {
    type: string;
    nodes: Record<string, number>;
    timeout?: TimeoutConfig;
    retries: number;
    retry_timeout?: number;
    pass_host: string;
    upstream_host?: string;
    scheme: string;
}

export interface TimeoutConfig {
    connect: number;
    send: number;
    read: number;
}

export interface Service {
    id: string;
    name: string;
    description: string;
    upstream?: InlineUpstream;
    upstream_id?: string;
    plugins: Record<string, any>;
    enable: boolean;
    labels: Record<string, string>;
    created_at?: string;
    updated_at?: string;
}

export interface Upstream {
    id: string;
    name: string;
    description: string;
    type: string;
    hash_on?: string;
    key?: string;
    nodes: Record<string, number>;
    retries: number;
    retry_timeout?: number;
    timeout?: TimeoutConfig;
    scheme: string;
    pass_host: string;
    upstream_host?: string;
    checks?: any;
    labels: Record<string, string>;
    created_at?: string;
    updated_at?: string;
}

export interface Consumer {
    id: string;
    username: string;
    description: string;
    plugins: Record<string, any>;
    group?: string;
    labels: Record<string, string>;
    created_at?: string;
    updated_at?: string;
}

export interface SslCert {
    id: string;
    snis: string[];
    cert: string;
    key: string;
    client_cert?: string;
    status: boolean;
    validity_end?: string;
    labels: Record<string, string>;
    created_at?: string;
    updated_at?: string;
}

export interface ListResponse<T> {
    total: number;
    list: T[];
}

export interface HealthResponse {
    status: string;
    version: string;
    cache: {
        routes: number;
        services: number;
        upstreams: number;
        consumers: number;
        ssl_certs: number;
        plugin_configs: number;
    };
    plugins_loaded: number;
}

// ─── Routes ───────────────────────────────────────────────────────

export const routes = {
    list: () => request<ListResponse<Route>>('/routes'),
    get: (id: string) => request<{ value: Route }>(`/routes/${id}`),
    create: (data: Partial<Route>) => request<{ value: Route }>('/routes', { method: 'POST', body: JSON.stringify(data) }),
    update: (id: string, data: Partial<Route>) => request<{ value: Route }>(`/routes/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    delete: (id: string) => request<void>(`/routes/${id}`, { method: 'DELETE' }),
};

// ─── Services ──────────────────────────────────────────────────────

export const services = {
    list: () => request<ListResponse<Service>>('/services'),
    get: (id: string) => request<{ value: Service }>(`/services/${id}`),
    create: (data: Partial<Service>) => request<{ value: Service }>('/services', { method: 'POST', body: JSON.stringify(data) }),
    update: (id: string, data: Partial<Service>) => request<{ value: Service }>(`/services/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    delete: (id: string) => request<void>(`/services/${id}`, { method: 'DELETE' }),
};

// ─── Upstreams ─────────────────────────────────────────────────────

export const upstreams = {
    list: () => request<ListResponse<Upstream>>('/upstreams'),
    get: (id: string) => request<{ value: Upstream }>(`/upstreams/${id}`),
    create: (data: Partial<Upstream>) => request<{ value: Upstream }>('/upstreams', { method: 'POST', body: JSON.stringify(data) }),
    update: (id: string, data: Partial<Upstream>) => request<{ value: Upstream }>(`/upstreams/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    delete: (id: string) => request<void>(`/upstreams/${id}`, { method: 'DELETE' }),
};

// ─── Consumers ─────────────────────────────────────────────────────

export const consumers = {
    list: () => request<ListResponse<Consumer>>('/consumers'),
    get: (id: string) => request<{ value: Consumer }>(`/consumers/${id}`),
    create: (data: Partial<Consumer>) => request<{ value: Consumer }>('/consumers', { method: 'POST', body: JSON.stringify(data) }),
    delete: (id: string) => request<void>(`/consumers/${id}`, { method: 'DELETE' }),
};

// ─── SSL ───────────────────────────────────────────────────────────

export const ssl = {
    list: () => request<ListResponse<SslCert>>('/ssls'),
    get: (id: string) => request<{ value: SslCert }>(`/ssls/${id}`),
    create: (data: Partial<SslCert>) => request<{ value: SslCert }>('/ssls', { method: 'POST', body: JSON.stringify(data) }),
    delete: (id: string) => request<void>(`/ssls/${id}`, { method: 'DELETE' }),
};

// ─── Plugins ───────────────────────────────────────────────────────

export const plugins = {
    list: () => request<ListResponse<string>>('/plugins/list'),
};

// ─── Health ────────────────────────────────────────────────────────

export const health = {
    check: () => request<HealthResponse>('/health'),
};

// ─── Metrics ───────────────────────────────────────────────────────

export const metrics = {
    get: async () => {
        const res = await fetch(`${BASE}/metrics`);
        return res.text();
    },
};
