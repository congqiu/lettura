# Optional render fallback: set `--build-arg RENDERING=0` to compile without
# chromiumoxide and skip installing chromium in the runtime image (image size
# drops from ~350MB to ~100MB).
ARG RENDERING=1

# Stage 1: Build frontend
FROM node:24 AS frontend-builder

RUN corepack enable && corepack prepare pnpm@10- --activate

WORKDIR /app/web
COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm run build

# Stage 2: Build Rust binary
# Pinning to -bookworm keeps the builder's glibc in sync with the
# debian:bookworm-slim runtime; rust:latest floats to a newer Debian whose
# glibc 2.39 symbols won't resolve at runtime.
FROM rust:bookworm AS backend-builder
ARG RENDERING

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY cli/Cargo.toml cli/Cargo.toml
COPY migrations/ migrations/
COPY src/ src/
COPY cli/src/ cli/src/
COPY skills/ skills/
COPY --from=frontend-builder /app/web/dist web/dist

# BuildKit cache mounts: registry caches downloaded crates,
# target caches incremental compilation outputs. Both survive across
# rebuilds in the same CI runner.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    if [ "$RENDERING" = "1" ]; then \
      cargo build --release --bin lettura && cp target/release/lettura /lettura; \
    else \
      cargo build --release --no-default-features --bin lettura && cp target/release/lettura /lettura; \
    fi

# Stage 2b: Unit test
# Usage:
#   docker build --target test -t lettura-test .
#   docker build --target test --build-arg TEST_ARGS="--lib search" -t lettura-test .
# The build fails if any test fails, which is intentional for CI.
# For local dev, use TEST_ARGS to filter: --build-arg TEST_ARGS="--lib search"
FROM backend-builder AS test
ARG TEST_ARGS=""
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    if [ "$RENDERING" = "1" ]; then \
      cargo test ${TEST_ARGS} -- --nocapture; \
    else \
      cargo test --no-default-features ${TEST_ARGS} -- --nocapture; \
    fi

# Stage 3: Runtime
FROM debian:bookworm-slim
ARG RENDERING

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && if [ "$RENDERING" = "1" ]; then \
         apt-get install -y --no-install-recommends \
           chromium \
           fonts-noto-cjk \
           fonts-noto-color-emoji; \
       fi \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for running the application
RUN groupadd -r lettura && useradd -r -g lettura -d /data -s /sbin/nologin lettura

WORKDIR /app

COPY --from=backend-builder /lettura ./lettura
COPY --from=backend-builder /app/migrations ./migrations

RUN mkdir -p /data/tantivy /data/site-configs /data/pages /data/storage \
    && chown -R lettura:lettura /data

USER lettura

EXPOSE 3330

CMD ["./lettura"]
