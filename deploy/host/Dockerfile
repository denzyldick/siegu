# syntax=docker/dockerfile:1

###############################################################################
# Stage 1 — build the Rust `siegu` binary (host: `siegu web`).
# Only -p siegu-cli (+ its siegu-core dep) is built; src-tauri (Tauri) is a
# workspace member but is deliberately excluded (native GUI toolchain).
###############################################################################
FROM rust:1-slim AS rust
WORKDIR /app

# System deps for siegu-core (heic, ffmpeg-next, rusqlite bundled w/ cc, openssl).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev libclang-dev clang ffmpeg curl \
    && rm -rf /var/lib/apt/lists/*

# Build context is the repo root; manifests + vendor patch are copied first.
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates/siegu-core/ crates/siegu-core/
COPY crates/siegu-cli/ crates/siegu-cli/
COPY crates/siegu-signal/ crates/siegu-signal/
# src-tauri is a workspace member + provides the [patch] for esaxx-rs.
# Only its manifest, `src/` (declared siegu_lib + main targets), and vendor/
# are needed so cargo can resolve the workspace; `-p siegu-cli` never builds
# the Tauri GUI crates. gen/ + test_models/ (2GB) are deliberately excluded.
COPY src-tauri/Cargo.toml src-tauri/Cargo.toml
COPY src-tauri/src/ src-tauri/src/
COPY src-tauri/vendor/ src-tauri/vendor/
COPY shared/ shared/
COPY crates/ crates/

# Configure the ffmpeg/linking env the core uses, then build just the CLI.
RUN cargo build -p siegu-cli --release --locked
RUN cp target/release/siegu /siegu

###############################################################################
# Stage 2 — build the Vue client (the real desktop UI, reused as the web UI).
###############################################################################
FROM node:22-alpine AS ui
WORKDIR /ui
COPY package.json package-lock.json* ./
# --ignore-scripts: the repo's `prepare` script (git config core.hooksPath)
# would otherwise run and fail (no git in this stage). `npm ci` installs
# exactly what package-lock.json pins (the committed bun.lock is npm-format and
# not bun-native, so --frozen-lockfile can't be used here).
RUN npm ci --ignore-scripts --prefer-offline
COPY index.html vite.config.ts tsconfig.json ./
COPY src/ src/
COPY public/ public/
COPY branding/ branding/
RUN npm run build

###############################################################################
# Stage 3 — runtime: the `siegu web` host + the Vue bundle.
###############################################################################
FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 siegu

COPY --from=rust /siegu /usr/local/bin/siegu
COPY --from=ui /ui/dist /srv/webui
RUN chown -R siegu:siegu /srv/webui

ENV SIEGU_WEB_DIST=/srv/webui HOME=/home/siegu
USER siegu

EXPOSE 8787
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -sf --max-time 4 -o /dev/null http://127.0.0.1:8787/ || exit 1

# `siegu web` serves the UI, embedded signalling, and the /ws bridge. Mount
# your indexed library/DB at /data to make it the host's library.
ENTRYPOINT ["siegu", "web", "--port", "8787", "--config", "/data", "--share-mode", "ro"]
