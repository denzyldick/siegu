# Build the standalone Siegu signalling server as a small distroless-ish image.
# Build context is the repository root (the crate is a workspace member).
#
# The builder is pinned to the bookworm (Debian 12) variant so the produced
# binary only requires glibc/libstdc++ symbols available in Debian 12; the
# floating `rust:1-slim` base has moved to Debian 13, which produces binaries
# that fail to load on the bookworm runtime below (GLIBC_2.38 / GLIBCXX_3.4.31).
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    protobuf-compiler \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY . .

# Build only the signalling binary. siegu-core is built with default-features
# disabled so ML/ort/tokenizers are never compiled.
RUN cargo build --release -p siegu-signal

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 -m siegu

USER siegu
WORKDIR /home/siegu

COPY --from=builder /app/target/release/siegu-signal /usr/local/bin/siegu-signal

ENV PORT=8080
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf --max-time 4 -o /dev/null "http://127.0.0.1:${PORT:-8080}/healthz" || exit 1

ENTRYPOINT ["siegu-signal"]
