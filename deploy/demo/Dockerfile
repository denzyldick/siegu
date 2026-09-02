# syntax=docker/dockerfile:1

###############################################################################
# Demo image — pre-seeded Siegu instance for try-before-you-buy.
# Based on deploy/host/Dockerfile but bakes in the demo library so the
# container starts with 46 items ready to browse (no scan needed).
###############################################################################

###############################################################################
# Stage 1 — build the Rust `siegu` binary.
###############################################################################
FROM rust:1-slim AS rust
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev libclang-dev clang ffmpeg curl \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates/siegu-core/ crates/siegu-core/
COPY crates/siegu-cli/ crates/siegu-cli/
COPY crates/siegu-signal/ crates/siegu-signal/
COPY src-tauri/Cargo.toml src-tauri/Cargo.toml
COPY src-tauri/src/ src-tauri/src/
COPY src-tauri/vendor/ src-tauri/vendor/
COPY shared/ shared/
COPY crates/ crates/

RUN cargo build -p siegu-cli --release --locked
RUN cp target/release/siegu /siegu

###############################################################################
# Stage 2 — build the Vue client.
###############################################################################
FROM node:22 AS ui
WORKDIR /ui
COPY package.json package-lock.json* ./
RUN npm ci --ignore-scripts --prefer-offline
COPY index.html vite.config.ts tsconfig.json ./
COPY src/ src/
COPY public/ public/
COPY branding/ branding/
COPY shared/ shared/
RUN npm run build

###############################################################################
# Stage 3 — runtime: siegu web + Vue bundle + baked-in demo data.
###############################################################################
FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /data && useradd --create-home --uid 10001 siegu

COPY --from=rust /siegu /usr/local/bin/siegu
COPY --from=ui /ui/dist /srv/webui

# Copy demo assets into the image.
COPY demos/ /demos/

# Seed the library at build time so the container starts pre-loaded.
# --demos-root tells the CLI where to find the asset directories.
RUN /usr/local/bin/siegu seed-demo --config /data --demos-root /demos \
    && chown -R siegu:siegu /data /srv/webui

ENV SIEGU_WEB_DIST=/srv/webui HOME=/home/siegu
USER siegu

EXPOSE 8787
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD curl -sf --max-time 4 -o /dev/null http://127.0.0.1:8787/ || exit 1

ENTRYPOINT ["siegu", "web", "--port", "8787", "--config", "/data", "--share-mode", "ro"]
