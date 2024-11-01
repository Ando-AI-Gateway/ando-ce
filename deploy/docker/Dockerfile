# ==========================================
# Ando API Gateway — Production Dockerfile
# ==========================================
# Multi-stage build for minimal image size.
# Build: docker build -t ando:latest -f deploy/docker/Dockerfile .
# Run:   docker run -p 9080:9080 -p 9180:9180 ando:latest

FROM rust:1.85-bookworm AS builder

WORKDIR /build

# Install musl tools for static linking
RUN apt-get update && apt-get install -y \
    musl-tools \
    cmake \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace
COPY . .

# Build release binary
RUN cargo build --release --bin ando

# ==========================================
# Runtime stage — minimal image
# ==========================================
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create ando user
RUN groupadd -r ando && useradd -r -g ando -d /etc/ando -s /sbin/nologin ando

# Copy binary
COPY --from=builder /build/target/release/ando /usr/local/bin/ando

# Copy default config and Lua PDK
COPY config/ando.yaml /etc/ando/ando.yaml
COPY lua/ /etc/ando/lua/

# Create plugin directory
RUN mkdir -p /etc/ando/plugins && chown -R ando:ando /etc/ando

# Set user
USER ando

# Expose ports
EXPOSE 9080 9443 9180

# Health check
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -sf http://localhost:9180/apisix/admin/health || exit 1

ENTRYPOINT ["ando"]
CMD ["--config", "/etc/ando/ando.yaml"]
