FROM rust:1.75-slim as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src/ src/

# Build the publisherd binary
RUN cargo build --release --bin publisherd

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false -m -d /app appuser

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/publisherd ./publisherd

# Change ownership to appuser
RUN chown -R appuser:appuser /app

USER appuser

CMD ["./publisherd"]