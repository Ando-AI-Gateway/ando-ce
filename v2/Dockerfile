# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#  Ando v2 — Multi-stage production build
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

FROM rust:latest AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    cmake \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml ./
COPY ando-core/ ando-core/
COPY ando-proxy/ ando-proxy/
COPY ando-plugin/ ando-plugin/
COPY ando-plugins/ ando-plugins/
COPY ando-store/ ando-store/
COPY ando-observability/ ando-observability/
COPY ando-admin/ ando-admin/
COPY ando-server/ ando-server/

# Build in release mode
RUN cargo build --release --bin ando-server

# ── Runtime image ──
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/ando-server /usr/local/bin/ando

# Default config (can be overridden via volume mount)
COPY config/ /etc/ando/

EXPOSE 9080 9443 9180

ENTRYPOINT ["/usr/local/bin/ando"]
CMD ["-c", "/etc/ando/ando.yaml"]
