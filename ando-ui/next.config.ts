import type { NextConfig } from "next";

const isProd = process.env.NODE_ENV === 'production';

const nextConfig: NextConfig = {
  ...(isProd ? { output: 'export', images: { unoptimized: true } } : {}),
  async rewrites() {
    if (isProd) return [];
    return [
      {
        source: '/apisix/admin/:path*',
        destination: 'http://localhost:9180/apisix/admin/:path*',
      },
      {
        source: '/metrics',
        destination: 'http://localhost:9180/metrics',
      },
    ];
  },
};

export default nextConfig;
