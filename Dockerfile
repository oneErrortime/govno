# ╔══════════════════════════════════════════════════════════════╗
# ║  💩 ГОВНО — Multistage Docker Build                         ║
# ║                                                              ║
# ║  ARG TARGET controls which binary to build:                 ║
# ║    --build-arg TARGET=govno-orchestrator                     ║
# ║    --build-arg TARGET=govno-worker-liquid                    ║
# ║    --build-arg TARGET=govno-worker-solid  etc.               ║
# ╚══════════════════════════════════════════════════════════════╝

# ── Stage 1: dep cache ────────────────────────────────────────────────────────
# Copy manifests only → compile empty stubs → cache this layer.
# Real source copy comes later; only changed crates re-compile.
FROM rust:1.77-slim AS deps

RUN apt-get update && \
    apt-get install -y --no-install-recommends pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy every Cargo.toml / Cargo.lock from the workspace
COPY Cargo.toml Cargo.lock ./
COPY proto/Cargo.toml              proto/Cargo.toml
COPY orchestrator/Cargo.toml       orchestrator/Cargo.toml
COPY workers/common/Cargo.toml     workers/common/Cargo.toml
COPY workers/liquid/Cargo.toml     workers/liquid/Cargo.toml
COPY workers/solid/Cargo.toml      workers/solid/Cargo.toml
COPY workers/gas/Cargo.toml        workers/gas/Cargo.toml
COPY workers/critical/Cargo.toml   workers/critical/Cargo.toml
# client is wasm-only — not built here
COPY client/Cargo.toml             client/Cargo.toml

# Create minimal stub sources so cargo can resolve and compile deps
RUN set -ex && \
    mkdir -p proto/src orchestrator/src \
             workers/common/src \
             workers/liquid/src workers/solid/src \
             workers/gas/src workers/critical/src \
             client/src && \
    echo 'pub fn placeholder() {}' > proto/src/lib.rs && \
    echo 'fn main() {}'            > orchestrator/src/main.rs && \
    echo 'pub fn placeholder() {}' > workers/common/src/lib.rs && \
    for w in liquid solid gas critical; do \
        echo 'fn main() {}' > workers/$w/src/main.rs; \
    done && \
    # client is cdylib — needs a lib.rs stub
    echo '' > client/src/lib.rs

# Build deps only (exclude client — wasm, not native)
RUN cargo build --release \
        -p govno-orchestrator \
        -p govno-worker-liquid \
        -p govno-worker-solid \
        -p govno-worker-gas \
        -p govno-worker-critical \
    && echo "✅ deps cached"

# ── Stage 2: build ────────────────────────────────────────────────────────────
FROM deps AS builder

ARG TARGET=govno-orchestrator

# Copy real sources
COPY proto/src             proto/src
COPY orchestrator/src      orchestrator/src
COPY workers/common/src    workers/common/src
COPY workers/liquid/src    workers/liquid/src
COPY workers/solid/src     workers/solid/src
COPY workers/gas/src       workers/gas/src
COPY workers/critical/src  workers/critical/src

# Touch sources so cargo detects the change
RUN find proto/src orchestrator/src workers -name '*.rs' -exec touch {} \;

RUN cargo build --release -p "${TARGET}" && \
    echo "✅ built ${TARGET}" && \
    ls -lh target/release/ | grep -E "^-rwx"

# ── Stage 3: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

ARG TARGET=govno-orchestrator

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates wget && \
    rm -rf /var/lib/apt/lists/* && \
    useradd -r -u 1001 -s /bin/false govno

COPY --from=builder /app/target/release/${TARGET} /usr/local/bin/app

USER govno
EXPOSE 3000

ENV RUST_LOG=info
ENV GOVNO_TOKEN=говно
ENV ORCHESTRATOR_URL=ws://orchestrator:3000/producer

HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
    CMD wget -qO- http://localhost:3000/health || exit 1

CMD ["/usr/local/bin/app"]
