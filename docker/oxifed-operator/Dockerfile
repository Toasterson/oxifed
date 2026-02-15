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

# Build the operator binary
RUN cargo build --release -p oxifed-operator --bin oxifed-operator

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user with explicit UID
RUN useradd -r -u 10001 -s /bin/false -m -d /app appuser

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/oxifed-operator ./oxifed-operator

# Change ownership to appuser
RUN chown -R appuser:appuser /app

USER 10001

CMD ["./oxifed-operator"]
