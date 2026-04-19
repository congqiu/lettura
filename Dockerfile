# Stage 1: Build frontend
FROM node:24 AS frontend-builder

RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app/web
COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm run build

# Stage 2: Build Rust binary
FROM rust:latest AS backend-builder

WORKDIR /app

# 2a: Cache Rust dependencies (rebuilds only when Cargo.toml/Cargo.lock change)
COPY Cargo.toml Cargo.lock ./
COPY migrations/ migrations/
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# 2b: Build actual application (only src/ changes invalidate this layer)
COPY src/ src/
COPY site-configs/ site-configs/
COPY --from=frontend-builder /app/web/dist web/dist
RUN touch src/main.rs && cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=backend-builder /app/target/release/lettura ./lettura
COPY --from=backend-builder /app/migrations ./migrations

RUN mkdir -p /data/tantivy

EXPOSE 3330

CMD ["./lettura"]
