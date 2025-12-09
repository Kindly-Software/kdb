# Multi-stage Dockerfile for Kindly-Verified WASM
# Stage 1: Build WASM bundle
# Stage 2: Runtime with Nginx

# ============================================================================
# STAGE 1: BUILD
# ============================================================================
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    nodejs \
    npm \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Rust WASM target
RUN rustup target add wasm32-unknown-unknown

# Install Trunk (WASM bundler)
RUN cargo install trunk

# Install wasm-opt for additional optimization (optional)
RUN npm install -g wasm-opt 2>/dev/null || true

# Set working directory
WORKDIR /build

# Copy Cargo files
COPY Cargo.toml Cargo.lock* ./

# Copy source code
COPY src ./src
COPY Trunk.toml .
COPY index.html .

# Build optimized WASM bundle
RUN --mount=type=cache,target=/build/target \
    cargo build --release --target wasm32-unknown-unknown && \
    trunk build --release

# Verify bundle size
RUN du -sh dist/*

# ============================================================================
# STAGE 2: RUNTIME
# ============================================================================
FROM nginx:alpine

# Install brotli support (for better compression)
RUN apk add --no-cache brotli

# Copy nginx configuration
COPY nginx.conf /etc/nginx/conf.d/default.conf

# Remove default Nginx files
RUN rm -rf /usr/share/nginx/html/*

# Copy built WASM bundle from builder
COPY --from=builder /build/dist /usr/share/nginx/html

# Create health check script
RUN echo '#!/bin/sh\ncurl -f http://localhost:8080/health || exit 1' > /healthcheck.sh && \
    chmod +x /healthcheck.sh

# Health check (Fly.io compatible)
HEALTHCHECK --interval=10s --timeout=5s --start-period=10s --retries=3 \
    CMD /healthcheck.sh

# Expose port
EXPOSE 8080

# Run Nginx in foreground
CMD ["nginx", "-g", "daemon off;"]

# ============================================================================
# METADATA
# ============================================================================
LABEL maintainer="Samuel <samuel@kindly.software>"
LABEL description="Kindly-Verified WASM - AI image detection with Byzantine design"
LABEL version="0.1.0"
