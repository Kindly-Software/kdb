# Multi-stage build for minimal production image
# Stage 1: Builder (Rust + build dependencies)
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkg-config \
    git

# Create build directory
WORKDIR /build

# Copy Cargo manifests
COPY Cargo.toml Cargo.lock* ./

# Copy source code
COPY src ./src
COPY benches ./benches
COPY tests ./tests
COPY examples ./examples

# Copy atomic_capsule dependency
COPY ../atomic_capsule ../atomic_capsule
COPY ../atomic_capsule_derive ../atomic_capsule_derive

# Build release binary (statically linked for Alpine)
# Note: Uses target x86_64-unknown-linux-musl for static linking
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release \
    --bin kdb \
    --target x86_64-unknown-linux-musl \
    2>&1 | head -100 || true && \
    find /build/target -name kdb -type f -executable -path "*/release/*" || \
    (echo "Release build not found, falling back to standard target" && \
     cargo build --release --bin kdb && \
     ls -la /build/target/release/kdb)

# ============================================================================
# Stage 2: Production image (minimal Alpine base)
# ============================================================================
FROM alpine:3.18

# Metadata
LABEL maintainer="Samuel <samuel@primitives.dev>"
LABEL description="KDB - The Kindly Debugger: T6 Mixed audit-compliant debugger"
LABEL version="0.1.0"

# Install minimal runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libc6-compat \
    libgcc

# Create non-root user for security
RUN addgroup -g 1000 kdb && \
    adduser -D -u 1000 -G kdb kdb

# Create data directory
RUN mkdir -p /var/lib/kdb && \
    chown -R kdb:kdb /var/lib/kdb && \
    chmod 700 /var/lib/kdb

# Create config directory
RUN mkdir -p /etc/kdb && \
    chown -R kdb:kdb /etc/kdb && \
    chmod 755 /etc/kdb

# Copy binary from builder (try musl target first, fall back to standard)
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/kdb /usr/local/bin/kdb || \
    COPY --from=builder /build/target/release/kdb /usr/local/bin/kdb

# Verify binary exists
RUN test -x /usr/local/bin/kdb || (echo "Binary not found!" && exit 1)

# Set ownership of binary
RUN chown kdb:kdb /usr/local/bin/kdb && chmod 755 /usr/local/bin/kdb

# Switch to non-root user
USER kdb

# Health check endpoint (every 10s, 2s timeout, 5s grace period)
HEALTHCHECK --interval=10s --timeout=2s --start-period=5s --retries=3 \
    CMD wget --quiet --tries=1 --spider http://localhost:8080/health || exit 1

# Expose MCP server port
EXPOSE 8080

# Set environment variables
ENV RUST_LOG=info
ENV KDB_DATA_DIR=/var/lib/kdb
ENV KDB_CONFIG_DIR=/etc/kdb

# Run MCP server
# Note: Requires atomic_mcp_server integration to expose MCP tools
ENTRYPOINT ["/usr/local/bin/kdb"]
CMD ["--help"]
