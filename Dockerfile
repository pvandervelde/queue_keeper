# Multi-stage Dockerfile for Queue-Keeper Service
#
# This Dockerfile implements the container architecture specified in
# specs/architecture/container-deployment.md:
# - Multi-stage build for minimal final image size
# - Non-root user for security
# - Health check integration
# - Debian bullseye-slim base for compatibility

# ============================================================================
# Stage 1: Builder
# ============================================================================
FROM rust:1.90-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/queue-keeper-core/Cargo.toml ./crates/queue-keeper-core/
COPY crates/queue-keeper-service/Cargo.toml ./crates/queue-keeper-service/
COPY crates/queue-keeper-cli/Cargo.toml ./crates/queue-keeper-cli/
COPY crates/github-bot-sdk/Cargo.toml ./crates/github-bot-sdk/
COPY crates/queue-runtime/Cargo.toml ./crates/queue-runtime/

# Create dummy source files to cache dependencies
RUN mkdir -p crates/queue-keeper-core/src \
    crates/queue-keeper-service/src \
    crates/queue-keeper-cli/src \
    crates/github-bot-sdk/src \
    crates/queue-runtime/src && \
    echo "fn main() {}" > crates/queue-keeper-service/src/main.rs && \
    echo "fn main() {}" > crates/queue-keeper-cli/src/main.rs && \
    echo "pub fn dummy() {}" > crates/queue-keeper-core/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/github-bot-sdk/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/queue-runtime/src/lib.rs

# Build dependencies only (this layer will be cached)
# Note: This will fail to link but will cache all external dependencies
RUN cargo build --release --package queue-keeper-service || true

# Remove dummy source files and build artifacts to force clean rebuild
RUN rm -rf crates/*/src && \
    rm -rf target/release/.fingerprint/queue-keeper-* && \
    rm -rf target/release/.fingerprint/queue-runtime-* && \
    rm -rf target/release/.fingerprint/github-bot-sdk-*

# Copy actual source code
COPY crates/ ./crates/

# Build the actual application with release optimizations
# See Cargo.toml for release profile settings:
# - opt-level = 3 (maximum optimization)
# - lto = true (link-time optimization)
# - codegen-units = 1 (better optimization)
# Force a clean rebuild of our crates (but keep cached dependencies)
RUN cargo build --release --package queue-keeper-service

# Strip debug symbols to reduce binary size
RUN strip /app/target/release/queue-keeper-service

# ============================================================================
# Stage 2: Runtime
# ============================================================================
FROM debian:bullseye-slim

# Install runtime dependencies (minimal set)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
# This follows security best practices and prevents privilege escalation
RUN useradd --create-home --shell /bin/bash --uid 1000 queuekeeper

# Set working directory
WORKDIR /home/queuekeeper

# Copy binary from builder stage
COPY --from=builder --chown=queuekeeper:queuekeeper \
    /app/target/release/queue-keeper-service ./queue-keeper-service

# Switch to non-root user
USER queuekeeper

# Expose HTTP port (default: 8080)
# Can be overridden via QUEUE_KEEPER_PORT environment variable
EXPOSE 8080

# Health check using existing /health endpoint
# Specifications from specs/architecture/container-deployment.md:
# - interval: 30s (check every 30 seconds)
# - timeout: 3s (fail if response takes longer)
# - start-period: 5s (grace period during startup)
# - retries: 3 (mark unhealthy after 3 consecutive failures)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Set default environment variables
# These can be overridden at runtime via -e flags or ConfigMaps
ENV QUEUE_KEEPER_LOG_LEVEL=info
ENV QUEUE_KEEPER_PORT=8080
ENV QUEUE_KEEPER_HOST=0.0.0.0
ENV RUST_BACKTRACE=1

# Entry point - the application handles SIGTERM/SIGINT gracefully
# See crates/queue-keeper-service/src/lib.rs for shutdown handling
ENTRYPOINT ["./queue-keeper-service"]
