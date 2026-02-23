# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  Ando CE — Multi-stage production build
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Build: docker build -t ando-v2:latest .
# Run:   docker run -p 9080:9080 -p 9180:9180 ando-v2:latest

FROM rust:latest AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    cmake \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy lockfile and workspace manifest first for layer caching
COPY Cargo.toml Cargo.lock ./

# Copy all crate manifests (for dependency caching)
COPY ando-core/Cargo.toml ando-core/Cargo.toml
COPY ando-proxy/Cargo.toml ando-proxy/Cargo.toml
COPY ando-plugin/Cargo.toml ando-plugin/Cargo.toml
COPY ando-plugins/Cargo.toml ando-plugins/Cargo.toml
COPY ando-store/Cargo.toml ando-store/Cargo.toml
COPY ando-observability/Cargo.toml ando-observability/Cargo.toml
COPY ando-admin/Cargo.toml ando-admin/Cargo.toml
COPY ando-server/Cargo.toml ando-server/Cargo.toml

# Create dummy sources for dependency pre-build (Docker layer cache)
RUN mkdir -p ando-core/src ando-proxy/src ando-plugin/src ando-plugins/src \
    ando-store/src ando-observability/src ando-admin/src ando-server/src && \
    echo "pub fn _dummy() {}" > ando-core/src/lib.rs && \
    echo "pub fn _dummy() {}" > ando-proxy/src/lib.rs && \
    echo "pub fn _dummy() {}" > ando-plugin/src/lib.rs && \
    echo "pub fn _dummy() {}" > ando-plugins/src/lib.rs && \
    echo "pub fn _dummy() {}" > ando-store/src/lib.rs && \
    echo "pub fn _dummy() {}" > ando-observability/src/lib.rs && \
    echo "pub fn _dummy() {}" > ando-admin/src/lib.rs && \
    echo "fn main() {}" > ando-server/src/main.rs

# Pre-build dependencies (cached unless Cargo.toml/Cargo.lock change)
RUN cargo build --release --bin ando-server 2>/dev/null || true

# Copy actual source code
COPY ando-core/ ando-core/
COPY ando-proxy/ ando-proxy/
COPY ando-plugin/ ando-plugin/
COPY ando-plugins/ ando-plugins/
COPY ando-store/ ando-store/
COPY ando-observability/ ando-observability/
COPY ando-admin/ ando-admin/
COPY ando-server/ ando-server/

# Touch source files to invalidate the dummy build cache
RUN find . -name "*.rs" -exec touch {} +

# Build in release mode
RUN cargo build --release --bin ando-server

# ── Runtime image ──
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r ando && useradd -r -g ando -d /etc/ando -s /sbin/nologin ando

COPY --from=builder /build/target/release/ando-server /usr/local/bin/ando

# Default config (can be overridden via volume mount)
COPY config/ /etc/ando/

RUN chown -R ando:ando /etc/ando

USER ando

EXPOSE 9080 9443 9180

# Health check via admin API
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:9180/apisix/admin/health || exit 1

ENTRYPOINT ["/usr/local/bin/ando"]
CMD ["-c", "/etc/ando/ando.yaml"]
