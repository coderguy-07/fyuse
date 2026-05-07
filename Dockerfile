# Multi-stage Dockerfile for Fuse
# Stage 1: Builder
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY config.toml.example config.yaml.example ./

# Build for release with optimizations
RUN cargo build --release --locked

# Stage 2: Runtime (Distroless)
FROM gcr.io/distroless/cc-debian12:nonroot

# Copy the binary from builder
COPY --from=builder /app/target/release/fuse /usr/local/bin/fuse

# Copy example configs
COPY --from=builder /app/config.toml.example /etc/fuse/config.toml.example
COPY --from=builder /app/config.yaml.example /etc/fuse/config.yaml.example

# Set user to nonroot
USER nonroot:nonroot

# Create volume mount points
VOLUME ["/data/models", "/data/cache", "/data/logs"]

# Expose default port
EXPOSE 8080

# Health check endpoint (will be implemented in the app)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/fuse", "health"]

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/fuse"]
CMD ["run"]
