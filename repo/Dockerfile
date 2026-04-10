# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1.88-slim AS builder

WORKDIR /app

# SQLite is bundled by sqlx — no libsqlite3-dev required at build or runtime.
# pkg-config is needed by some crates to locate system headers.
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# ── Dependency caching layer ──────────────────────────────────────────────────
# Copy both manifests so Cargo can pin exact versions from the lockfile.
COPY Cargo.toml Cargo.lock ./

# Compile a stub binary to warm the dependency cache independently of
# application source changes.
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release 2>&1 | tail -1 || true

# Purge stub artefacts so the real source triggers a full recompile.
RUN rm -rf src \
        target/release/deps/motorsport_backend* \
        target/release/motorsport_backend* \
        target/release/.fingerprint/motorsport-backend*

# ── Application build ─────────────────────────────────────────────────────────
COPY src ./src
# Refresh dependency versions against the running toolchain so any MSRV
# mismatch between the committed Cargo.lock and the builder image is
# resolved automatically before the real compile.
RUN cargo update
RUN cargo build --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Non-root user for least-privilege execution.
RUN groupadd -r motorsport && useradd -r -g motorsport motorsport

WORKDIR /app

# Copy only the compiled binary.
COPY --from=builder /app/target/release/motorsport-backend /app/motorsport-backend

# Install wget for the HEALTHCHECK command.
RUN apt-get update && apt-get install -y --no-install-recommends wget \
    && rm -rf /var/lib/apt/lists/*

# Create runtime directories.
#   /app/data    — SQLite database file  (named volume)
#   /app/logs    — structured JSON logs  (named volume)
#   /app/backups — nightly backup files  (named volume)
RUN mkdir -p /app/data /app/logs /app/backups \
    && chown -R motorsport:motorsport /app

USER motorsport

# ── Runtime environment defaults ──────────────────────────────────────────────
# All values are overridable via docker-compose env_file or -e flags.
ENV HOST=0.0.0.0
ENV PORT=8000
ENV LOG_LEVEL=info
ENV DATABASE_URL=sqlite:///app/data/motorsport.db
ENV BACKUP_DIR=/app/backups
ENV BACKUP_RETAIN_DAYS=7

EXPOSE 8000

# Uses $PORT so the check honours any port override without rebuilding the image.
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD wget -qO- "http://localhost:${PORT}/health" > /dev/null || exit 1

CMD ["/app/motorsport-backend"]
