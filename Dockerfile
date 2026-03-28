# Stage 1: Build frontend
FROM node:22-alpine AS frontend-builder

WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# Stage 2: Build Rust binary
FROM rust:latest AS backend-builder

WORKDIR /app

# Copy manifests first for better layer caching
COPY Cargo.toml ./
COPY Cargo.lock* ./

# Copy source code
COPY src/ src/
COPY migrations/ migrations/

# Copy built frontend assets from stage 1
COPY --from=frontend-builder /app/web/dist web/dist

# Build release binary
RUN cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled binary
COPY --from=backend-builder /app/target/release/lettura ./lettura

# Copy migrations (for SQLx runtime migrations)
COPY --from=backend-builder /app/migrations ./migrations

# Create data directory for tantivy index
RUN mkdir -p /data/tantivy

EXPOSE 3000

CMD ["./lettura"]
