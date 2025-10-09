# Ultra-simple Dockerfile for Render
FROM rust:1.83-slim as builder

WORKDIR /app

# Install build deps
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy only what we need
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build with verbose output
RUN cargo build --release --bin simple_trading_engine --verbose

# Runtime
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/simple_trading_engine ./simple_trading_engine

# Copy web assets
COPY --from=builder /app/src/web ./src/web

# Make executable
RUN chmod +x ./simple_trading_engine

# Test binary exists
RUN ls -lah ./simple_trading_engine

EXPOSE 8080
ENV RUST_LOG=info
ENV PORT=8080

CMD ["./simple_trading_engine"]
