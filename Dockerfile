# Build stage
FROM rust:1.83 as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release --bin simple_trading_engine

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1000 app

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/simple_trading_engine /app/simple_trading_engine

# Copy web assets
COPY src/web /app/src/web

# Set ownership
RUN chown -R app:app /app

USER app

# Expose port
EXPOSE 8080

# Set environment
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Run the application
CMD ["./simple_trading_engine"]
