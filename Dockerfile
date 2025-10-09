# Build stage
FROM rust:1.83-slim as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Use minimal dependencies for faster builds
COPY Cargo-minimal.toml Cargo.toml
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy actual source
COPY src ./src
RUN cargo build --release --bin simple_trading_engine

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/simple_trading_engine /app/simple_trading_engine

# Copy web assets
COPY src/web /app/src/web

# Expose port
EXPOSE 8080

# Set environment
ENV RUST_LOG=info
ENV PORT=8080

# Run the application
CMD ["./simple_trading_engine"]
