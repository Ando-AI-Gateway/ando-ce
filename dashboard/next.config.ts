import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "export",
  // All assets served from /dashboard/ on the admin API
  basePath: "/dashboard",
  // Static export — no image optimisation server
  images: { unoptimized: true },
  // trailing slash so /dashboard/routes/ resolves to routes/index.html
  trailingSlash: true,
};

export default nextConfig;
