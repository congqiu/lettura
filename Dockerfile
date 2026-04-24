# Optional render fallback: set `--build-arg RENDERING=0` to compile without
# chromiumoxide and skip installing chromium in the runtime image (image size
# drops from ~350MB to ~100MB).
ARG RENDERING=1

# Stage 1: Build frontend
FROM node:24 AS frontend-builder

RUN corepack enable && corepack prepare pnpm@latest --activate

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

# 2a: Cache Rust dependencies (rebuilds only when Cargo.toml/Cargo.lock change)
COPY Cargo.toml Cargo.lock ./
COPY cli/Cargo.toml cli/Cargo.toml
COPY migrations/ migrations/
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    mkdir -p cli/src && echo "fn main() {}" > cli/src/main.rs
RUN if [ "$RENDERING" = "1" ]; then \
      cargo build --release 2>/dev/null || true; \
    else \
      cargo build --release --no-default-features 2>/dev/null || true; \
    fi
RUN rm -rf src cli/src

# 2b: Build actual application (only src/ changes invalidate this layer)
COPY src/ src/
COPY cli/src/ cli/src/
COPY --from=frontend-builder /app/web/dist web/dist
RUN touch src/main.rs && \
    if [ "$RENDERING" = "1" ]; then \
      cargo build --release; \
    else \
      cargo build --release --no-default-features; \
    fi

# Stage 3: Runtime
FROM debian:bookworm-slim
ARG RENDERING

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && if [ "$RENDERING" = "1" ]; then \
         apt-get install -y --no-install-recommends \
           chromium \
           fonts-noto-cjk \
           fonts-noto-color-emoji; \
       fi \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=backend-builder /app/target/release/lettura ./lettura
COPY --from=backend-builder /app/migrations ./migrations

RUN mkdir -p /data/tantivy /data/site-configs

EXPOSE 3330

CMD ["./lettura"]
