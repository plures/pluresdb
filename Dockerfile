# Multi-stage build: Rust binary for PluresDB v3.0.0

FROM rust:1.85-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin pluresdb

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/pluresdb /usr/local/bin/pluresdb

# Default port: 34567 (API)
EXPOSE 34567

CMD ["pluresdb", "serve", "--port", "34567"]
