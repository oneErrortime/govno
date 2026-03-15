# ╔══════════════════════════════════════════════════════╗
# ║  💩 ГОВНО-СЕРВЕР  — Multistage Docker Build         ║
# ║                                                      ║
# ║  Stage 1 (builder): Rust + cargo → binary            ║
# ║  Stage 2 (runtime): debian-slim, non-root user       ║
# ╚══════════════════════════════════════════════════════╝

# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1.77-slim AS builder

RUN apt-get update && \
    apt-get install -y --no-install-recommends pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Кешируем зависимости отдельно от исходников.
# Копируем только Cargo.toml/lock, компилируем заглушки.
COPY Cargo.toml Cargo.lock ./
COPY server/Cargo.toml  server/Cargo.toml
COPY client/Cargo.toml  client/Cargo.toml

RUN mkdir -p server/src client/src && \
    echo 'fn main() {}' > server/src/main.rs && \
    echo ''              > client/src/lib.rs  && \
    cargo build -p govno-server --release     && \
    rm -rf server/src client/src

# Теперь копируем настоящий код и пересобираем только его
COPY server/src server/src
RUN touch server/src/main.rs && \
    cargo build -p govno-server --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/* && \
    # non-root пользователь
    useradd -r -u 1001 -s /bin/false govno

COPY --from=builder /app/target/release/govno-server /usr/local/bin/govno-server

USER govno
EXPOSE 3000

ENV GOVNO_TOKEN=говно
ENV RUST_LOG=govno_server=info

HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
    CMD wget -qO- http://localhost:3000/health || exit 1

CMD ["govno-server"]
