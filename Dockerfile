# syntax=docker/dockerfile:1.7
#
# outl Home Assistant addon: a headless outl host that serves the browser UI
# over HTTP, holds the iroh P2P endpoint, exposes the MCP server, and shares
# the workspace volume with the rest of Home Assistant.
#
# Two binaries are built:
#   - outl-server : the HTTP host (UI + command surface + events + fs API)
#   - outl        : the CLI, run as `outl mcp serve` by the server's MCP bridge
#
# The build compiles the whole outl workspace (iroh, wasmtime, tauri) because
# the server reuses `outl-tauri-shared`'s command bodies. That is why the
# build stage carries the Tauri Linux system libraries and the aws-lc-sys C
# toolchain.

########################################
# Build
########################################
FROM rust:1.95.0-slim-bookworm AS builder

# System deps:
#  - build-essential, cmake, perl : aws-lc-sys (iroh's rustls crypto backend)
#  - libwebkit2gtk-4.1-dev, librsvg2-dev, libssl-dev, libxdo-dev,
#    libayatana-appindicator3-dev : tauri (pulled in by outl-tauri-shared)
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential cmake perl \
      libwebkit2gtk-4.1-dev \
      librsvg2-dev \
      libssl-dev \
      libxdo-dev \
      libayatana-appindicator3-dev \
      pkg-config \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

# Build both binaries from the outl workspace root (the server crate is a
# workspace member at crates/outl-server). The cache mounts make rebuilds
# incremental. `strip` keeps the server image small (no debug info needed).
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/build/vendor/outl/target,sharing=locked \
    cd /build/vendor/outl \
    && cargo build --release -p outl-server -p outl-cli \
    && install -Dm755 target/release/outl-server /out/outl-server \
    && install -Dm755 target/release/outl /out/outl \
    && strip /out/outl-server /out/outl

########################################
# Runtime
########################################
FROM debian:bookworm-slim AS runtime

# ca-certificates for the iroh relay's TLS. Nothing else is linked.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 1000 outl \
 && useradd --uid 1000 --gid 1000 --home-dir /home/outl --create-home outl

COPY --from=builder /out/outl-server /usr/local/bin/outl-server
COPY --from=builder /out/outl /usr/local/bin/outl
COPY frontend/dist /app/static
COPY docker/entrypoint.sh /usr/local/bin/outl-entrypoint

# $HOME locates the iroh identity (~/.outl/identity.key, which IS this
# device's node id); $XDG_CONFIG_HOME locates the device store
# (~/.config/outl, which owns the actor bindings). Both must outlive the
# container — see the VOLUME below.
#
# Deliberately NOT $OUTL_DEVICE_DIR: that means "throwaway actor" and would
# rotate the node id, breaking every pairing.
ENV HOME=/home/outl \
    XDG_CONFIG_HOME=/home/outl/.config \
    OUTL_WORKSPACE=/data \
    OUTL_PORT=8090 \
    OUTL_STATIC_DIR=/app/static \
    RUST_LOG=info

RUN mkdir -p /data /home/outl/.outl /home/outl/.config/outl \
 && chown -R outl:outl /data /home/outl

# Two volumes, two for a reason: /data is replicated to every peer (the
# notes); /home/outl is what makes this container one device rather than a
# new one on every restart (the identity + actor bindings).
VOLUME ["/data", "/home/outl"]

USER outl
WORKDIR /data
STOPSIGNAL SIGTERM

ENTRYPOINT ["/usr/local/bin/outl-entrypoint"]
CMD ["outl-server"]
