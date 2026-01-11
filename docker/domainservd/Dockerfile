FROM rustlang/rust:nightly-bookworm as builder

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
COPY test-data/ test-data/

# Build the domainservd binary
RUN cargo build --release -p domainservd --bin domainservd

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
COPY --from=builder /app/target/release/domainservd ./domainservd

# Change ownership to appuser
RUN chown -R appuser:appuser /app

USER appuser

EXPOSE 8080

CMD ["./domainservd"]
