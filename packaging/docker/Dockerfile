# Multi-stage build for PluresDB
# Stage 1: Build Svelte UI
FROM node:20-alpine AS webbuild
WORKDIR /app/web
COPY web/svelte/ ./svelte/
RUN cd svelte && npm ci --no-fund --no-audit --only=production && npm run build

# Stage 2: Build Rust binary (if available)
FROM rust:1.75-alpine AS rustbuild
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY pluresdb-core/ ./pluresdb-core/
COPY pluresdb-cli/ ./pluresdb-cli/
COPY pluresdb-storage/ ./pluresdb-storage/
COPY pluresdb-network/ ./pluresdb-network/
COPY pluresdb-api/ ./pluresdb-api/
RUN cargo build --release

# Stage 3: Final runtime image
FROM denoland/deno:alpine-2.4.2

# Install runtime dependencies
RUN apk add --no-cache \
    sqlite \
    ca-certificates \
    tzdata

# Create app user for security
RUN addgroup -g 1001 -S pluresdb && \
    adduser -S pluresdb -u 1001 -G pluresdb

# Set working directory
WORKDIR /app

# Copy application files
COPY . .
COPY --from=webbuild /app/web/dist ./web/dist

# Copy Rust binary if built
COPY --from=rustbuild /app/target/release/pluresdb ./bin/pluresdb

# Create data directory and set permissions
RUN mkdir -p /app/data && \
    chown -R pluresdb:pluresdb /app

# Switch to non-root user
USER pluresdb

# Expose ports (API and Web UI)
EXPOSE 34567 34568

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD deno run -A --allow-net src/healthcheck.ts || exit 1

# Default command
CMD ["deno", "run", "-A", "--unstable-kv", "src/main.ts", "serve", "--port", "34567", "--web-port", "34568"]
