# Multi-stage build for minimal final image
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install required runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/trading-engine /usr/local/bin/

# Run the application
CMD ["trading-engine"]
