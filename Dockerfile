# ╔══════════════════════════════════════════════════════════════╗
# ║  💩 ГОВНО — Multistage Docker Build                         ║
# ║                                                              ║
# ║  Stage 1 (deps):    stub sources → compile all deps → cache ║
# ║  Stage 2 (builder): real sources → compile target binary    ║
# ║  Stage 3 (runtime): debian-slim, non-root, HEALTHCHECK      ║
# ║                                                              ║
# ║  Build args:                                                 ║
# ║    TARGET  — binary to build (default: govno-orchestrator)  ║
# ║              govno-orchestrator                              ║
# ║              govno-worker-liquid                             ║
# ║              govno-worker-solid                              ║
# ║              govno-worker-gas                                ║
# ║              govno-worker-critical                           ║
# ╚══════════════════════════════════════════════════════════════╝

# ── Stage 1: dependency cache ─────────────────────────────────────────────────
# Only Cargo manifests are copied here.
# Empty stub sources are compiled so cargo can resolve + download all deps.
# This layer is invalidated only when Cargo.lock or a Cargo.toml changes.
FROM rust:1.77-slim AS deps

ARG DEBIAN_FRONTEND=noninteractive
RUN set -ex \
    && apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/* \
    && echo "✅ system deps installed"

RUN rustc --version && cargo --version && rustup show

WORKDIR /app

# ── Copy ALL Cargo manifests (but not src/) ──────────────────────────────────
# Order matters: workspace root first, then members.
COPY Cargo.toml Cargo.lock ./

# Workspace members — each needs its Cargo.toml present before the stub build
COPY proto/Cargo.toml              proto/Cargo.toml
COPY orchestrator/Cargo.toml       orchestrator/Cargo.toml
COPY workers/common/Cargo.toml     workers/common/Cargo.toml
COPY workers/liquid/Cargo.toml     workers/liquid/Cargo.toml
COPY workers/solid/Cargo.toml      workers/solid/Cargo.toml
COPY workers/gas/Cargo.toml        workers/gas/Cargo.toml
COPY workers/critical/Cargo.toml   workers/critical/Cargo.toml
# client is cdylib (wasm-only) — NOT built in this Dockerfile.
# Its Cargo.toml must exist in workspace for resolver to work.
COPY client/Cargo.toml             client/Cargo.toml

# ── Create minimal stub sources ───────────────────────────────────────────────
# cargo build needs a compilable source file for every workspace member.
# We build stub versions of all native crates so their deps get cached.
# client stays as an empty lib.rs — it's excluded from the build below.
RUN set -ex \
    && mkdir -p \
        proto/src \
        orchestrator/src \
        workers/common/src \
        workers/liquid/src \
        workers/solid/src \
        workers/gas/src \
        workers/critical/src \
        client/src \
    && printf 'pub fn _stub() {}' > proto/src/lib.rs \
    && printf 'fn main() {}' > orchestrator/src/main.rs \
    && printf 'pub fn _stub() {}' > workers/common/src/lib.rs \
    && for w in liquid solid gas critical; do \
         printf 'fn main() {}' > workers/$w/src/main.rs; \
       done \
    # client is cdylib — needs a valid lib.rs stub but is excluded from build
    && printf '' > client/src/lib.rs \
    && echo "✅ stub sources created" \
    && find . -name "*.rs" | sort

# ── Compile deps only (native crates, excludes client) ───────────────────────
# --locked ensures Cargo.lock is used as-is (reproducible builds in Docker)
RUN set -ex \
    && echo "=== Compiling deps for all native crates ===" \
    && cargo build --release --locked \
        -p govno-orchestrator \
        -p govno-worker-liquid \
        -p govno-worker-solid \
        -p govno-worker-gas \
        -p govno-worker-critical \
    && echo "=== Dep cache stage complete ===" \
    && echo "Artifacts:" \
    && find target/release -maxdepth 1 -type f -executable | sort

# ── Stage 2: build ────────────────────────────────────────────────────────────
# Copy real sources on top of stubs. Touch them so cargo detects the change.
FROM deps AS builder

# ENV is NOT guaranteed to propagate across multi-stage FROM in all Docker
# versions/drivers. Re-export explicitly so cargo/rustc are on PATH.
# (rust:1.77-slim sets these in its own Dockerfile, but intermediate stages
# may not inherit ENV — explicit is safer and documents the dependency.)
ENV PATH="/usr/local/cargo/bin:${PATH}"
ENV CARGO_HOME="/usr/local/cargo"
ENV RUSTUP_HOME="/usr/local/rustup"

ARG TARGET=govno-orchestrator

# Verify cargo is reachable before doing anything else in this stage.
# Exit immediately (with path info) if not found.
RUN set -ex \
    && echo "=== builder stage: verifying cargo ===" \
    && which cargo   || { echo "❌ cargo not in PATH=$PATH"; exit 127; } \
    && cargo --version \
    && rustc --version \
    && echo "=== cargo OK ==="

COPY proto/src               proto/src
COPY orchestrator/src        orchestrator/src
COPY workers/common/src      workers/common/src
COPY workers/liquid/src      workers/liquid/src
COPY workers/solid/src       workers/solid/src
COPY workers/gas/src         workers/gas/src
COPY workers/critical/src    workers/critical/src

# Touch all .rs files so cargo sees them as newer than the stub artifacts
RUN find proto/src orchestrator/src workers -name '*.rs' -exec touch {} + \
    && echo "=== Source files for $TARGET ===" \
    && find . -path './target' -prune -o -name '*.rs' -print | sort

RUN set -ex \
    && echo "=== Building $TARGET ===" \
    && cargo build --release --locked -p "${TARGET}" \
    && echo "=== Build complete ===" \
    && echo "Binary:" \
    && ls -lh "target/release/${TARGET}" \
    && file "target/release/${TARGET}"

# ── Stage 3: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

ARG TARGET=govno-orchestrator

RUN set -ex \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 1001 -s /bin/false govno \
    && echo "✅ runtime deps installed"

COPY --from=builder /app/target/release/${TARGET} /usr/local/bin/app

# Verify binary is runnable before baking image
RUN /usr/local/bin/app --version 2>/dev/null || \
    /usr/local/bin/app --help    2>/dev/null || \
    echo "(binary has no --version flag, that is OK)"

USER govno
EXPOSE 3000

ENV RUST_LOG=info
ENV GOVNO_TOKEN=говно
ENV ORCHESTRATOR_URL=ws://orchestrator:3000/producer
ENV LISTEN_ADDR=0.0.0.0:3000

# wget is available in this image (installed above) — used by HEALTHCHECK
HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
    CMD wget -qO- http://localhost:3000/health || exit 1

CMD ["/usr/local/bin/app"]
